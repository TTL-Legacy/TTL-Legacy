import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { DrStatusWidget } from '../components/DrStatusWidget';
import type { DrStatusResponse } from '../api/client';

describe('DrStatusWidget', () => {
  it('renders nothing for non-admin users', () => {
    const fetchStatus = jest.fn();
    const { container } = render(
      <DrStatusWidget isAdmin={false} fetchStatus={fetchStatus} />,
    );

    expect(container.firstChild).toBeNull();
    expect(fetchStatus).not.toHaveBeenCalled();
  });

  it('displays healthy DR status with backup timestamp, verification result, and no alerts', async () => {
    const mockStatus: DrStatusResponse = {
      last_successful_backup_at: '2026-11-23T20:00:00.000Z',
      last_restore_verification: {
        status: 'passed',
        verified_at: '2026-11-23T21:00:00.000Z',
        backup_prefix: 'backups/20261123T200000Z',
        details: 'All checksums and GPG signatures verified.',
      },
      active_alerts: [],
      checked_at: '2026-11-23T22:00:00.000Z',
    };

    const fetchStatus = jest.fn().mockResolvedValue(mockStatus);

    render(<DrStatusWidget isAdmin fetchStatus={fetchStatus} />);

    await waitFor(() => {
      expect(screen.getByTestId('dr-last-backup')).toHaveTextContent(
        '2026-11-23T20:00:00.000Z',
      );
    });

    expect(screen.getByTestId('dr-restore-verification')).toHaveTextContent('Passed');
    expect(screen.getByTestId('dr-restore-verification')).toHaveTextContent(
      'backups/20261123T200000Z',
    );
    expect(screen.getByTestId('dr-alerts-empty')).toHaveTextContent('No active DR alerts.');
  });

  it('displays active DR alerts and failed restore verification', async () => {
    const mockStatus: DrStatusResponse = {
      last_successful_backup_at: null,
      last_restore_verification: {
        status: 'failed',
        verified_at: '2026-11-23T21:30:00.000Z',
        backup_prefix: 'backups/20261122T120000Z',
        details: 'GPG signature verification failed.',
      },
      active_alerts: [
        {
          id: 'backup-missing',
          severity: 'critical',
          message: 'No successful backup has been recorded yet.',
          triggered_at: '2026-11-23T22:00:00.000Z',
        },
        {
          id: 'replica-lag',
          severity: 'warning',
          message: 'Cross-region replica is 15 minutes behind.',
          triggered_at: '2026-11-23T21:55:00.000Z',
        },
      ],
      checked_at: '2026-11-23T22:00:00.000Z',
    };

    const fetchStatus = jest.fn().mockResolvedValue(mockStatus);

    render(<DrStatusWidget isAdmin fetchStatus={fetchStatus} />);

    await waitFor(() => {
      expect(screen.getByTestId('dr-last-backup')).toHaveTextContent(
        'No successful backup recorded',
      );
    });

    expect(screen.getByTestId('dr-restore-verification')).toHaveTextContent('Failed');
    const alertsList = screen.getByTestId('dr-alerts-list');
    expect(alertsList).toHaveTextContent('[CRITICAL]');
    expect(alertsList).toHaveTextContent('No successful backup has been recorded yet.');
    expect(alertsList).toHaveTextContent('[WARNING]');
    expect(alertsList).toHaveTextContent('Cross-region replica is 15 minutes behind.');
  });

  it('shows an error alert when fetching fails and supports refreshing', async () => {
    const fetchStatus = jest
      .fn()
      .mockRejectedValueOnce(new Error('valid admin API key required'))
      .mockResolvedValueOnce({
        last_successful_backup_at: '2026-11-23T20:00:00.000Z',
        last_restore_verification: {
          status: 'unknown',
          verified_at: null,
          backup_prefix: null,
          details: null,
        },
        active_alerts: [],
        checked_at: '2026-11-23T22:00:00.000Z',
      });

    render(<DrStatusWidget isAdmin fetchStatus={fetchStatus} />);

    await waitFor(() => {
      expect(screen.getByTestId('dr-status-error')).toHaveTextContent(
        'valid admin API key required',
      );
    });

    fireEvent.click(screen.getByRole('button', { name: /refresh/i }));

    await waitFor(() => {
      expect(screen.getByTestId('dr-last-backup')).toHaveTextContent(
        '2026-11-23T20:00:00.000Z',
      );
    });
    expect(fetchStatus).toHaveBeenCalledTimes(2);
  });
});
