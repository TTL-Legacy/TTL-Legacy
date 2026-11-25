import React from 'react';
import { render, screen, act } from '@testing-library/react';
import { TtlCountdown } from '../components/TtlCountdown';

const NOW_S = 1_700_000_000;

beforeEach(() => {
  jest.useFakeTimers();
  jest.setSystemTime(NOW_S * 1000);
});

afterEach(() => {
  jest.useRealTimers();
});

describe('TtlCountdown', () => {
  it('renders the formatted remaining time', () => {
    const expiresAt = NOW_S + 7260; // 2h 1m 0s
    render(<TtlCountdown expiresAt={expiresAt} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveTextContent('2h 1m 0s');
  });

  it('renders "Expired" when the TTL has already elapsed', () => {
    render(<TtlCountdown expiresAt={NOW_S - 100} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveTextContent('Expired');
  });

  it('ticks every second', () => {
    const expiresAt = NOW_S + 120; // 2m 0s
    render(<TtlCountdown expiresAt={expiresAt} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveTextContent('2m 0s');

    act(() => { jest.advanceTimersByTime(1000); });
    expect(screen.getByTestId('ttl-countdown')).toHaveTextContent('1m 59s');
  });

  it('shows "ok" color when above warning threshold', () => {
    const expiresAt = NOW_S + 90000; // > 86400
    render(<TtlCountdown expiresAt={expiresAt} warningThreshold={86400} criticalThreshold={3600} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveStyle({ color: '#2e7d32' });
  });

  it('shows "warning" color within warning threshold', () => {
    const expiresAt = NOW_S + 7200; // 2h, within 24h warning, above 1h critical
    render(<TtlCountdown expiresAt={expiresAt} warningThreshold={86400} criticalThreshold={3600} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveStyle({ color: '#ed6c02' });
  });

  it('shows "critical" color within critical threshold', () => {
    const expiresAt = NOW_S + 1800; // 30m, within 1h critical
    render(<TtlCountdown expiresAt={expiresAt} warningThreshold={86400} criticalThreshold={3600} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveStyle({ color: '#d32f2f' });
  });

  it('shows "expired" color when already expired', () => {
    render(<TtlCountdown expiresAt={NOW_S - 1} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveStyle({ color: '#757575' });
  });

  it('renders an accessible time element with aria-label', () => {
    const expiresAt = NOW_S + 3600;
    render(<TtlCountdown expiresAt={expiresAt} />);
    const el = screen.getByRole('timer');
    expect(el).toHaveAttribute('aria-label');
    expect(el.getAttribute('aria-label')).toMatch(/Time until check-in required/i);
  });

  it('formats days correctly for long TTLs', () => {
    const expiresAt = NOW_S + 86400 * 3 + 3600 * 2 + 60 * 5; // 3d 2h 5m
    render(<TtlCountdown expiresAt={expiresAt} />);
    expect(screen.getByTestId('ttl-countdown')).toHaveTextContent('3d 2h 5m');
  });
});
