import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CheckInButton } from '../components/CheckInButton';

// Mock wallet hook
jest.mock('../hooks/useWallet', () => ({
  useWallet: jest.fn(() => ({
    isConnected: true,
    address: 'GTEST123456789',
    signTransaction: jest.fn().mockResolvedValue({ hash: 'tx123' }),
  })),
}));

// Mock transaction API
const mockSubmitCheckIn = jest.fn();
jest.mock('../api/vault', () => ({
  submitCheckIn: mockSubmitCheckIn,
}));

describe('CheckInButton', () => {
  const mockOnSuccess = jest.fn();
  const vaultId = 'vault-123';

  beforeEach(() => {
    jest.clearAllMocks();
    mockSubmitCheckIn.mockResolvedValue({ success: true, ttlExtended: true });
  });

  it('renders the check-in button', () => {
    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });
    expect(button).toBeInTheDocument();
  });

  it('shows loading state while transaction is pending', async () => {
    mockSubmitCheckIn.mockImplementation(
      () => new Promise(resolve => setTimeout(() => resolve({ success: true }), 100))
    );

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });

    await userEvent.click(button);

    expect(screen.getByText(/pending/i)).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByText(/pending/i)).not.toBeInTheDocument();
    });
  });

  it('shows success message when check-in succeeds', async () => {
    mockSubmitCheckIn.mockResolvedValue({ success: true, ttlExtended: true });

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });

    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(/success/i)).toBeInTheDocument();
    });
  });

  it('shows failure message when check-in fails', async () => {
    mockSubmitCheckIn.mockResolvedValue({ success: false, error: 'Transaction failed' });

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });

    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(/failed/i)).toBeInTheDocument();
    });
  });

  it('calls onSuccess callback when check-in succeeds', async () => {
    mockSubmitCheckIn.mockResolvedValue({ success: true, ttlExtended: true });

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });

    await userEvent.click(button);

    await waitFor(() => {
      expect(mockOnSuccess).toHaveBeenCalled();
    });
  });

  it('shows transaction hash when check-in succeeds', async () => {
    const txHash = 'abc123def456';
    mockSubmitCheckIn.mockResolvedValue({ success: true, txHash });

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });

    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(new RegExp(txHash))).toBeInTheDocument();
    });
  });

  it('disables button while transaction is in progress', async () => {
    mockSubmitCheckIn.mockImplementation(
      () => new Promise(resolve => setTimeout(() => resolve({ success: true }), 100))
    );

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });

    await userEvent.click(button);

    expect(button).toBeDisabled();
  });

  it('shows error message for network failures', async () => {
    mockSubmitCheckIn.mockRejectedValue(new Error('Network error'));

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.getByRole('button', { name: /check in/i });

    await userEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(/error/i)).toBeInTheDocument();
    });
  });

  it('requires wallet connection', () => {
    jest.resetModules();
    jest.mock('../hooks/useWallet', () => ({
      useWallet: jest.fn(() => ({
        isConnected: false,
        address: null,
      })),
    }));

    render(<CheckInButton vaultId={vaultId} onSuccess={mockOnSuccess} />);
    const button = screen.queryByRole('button', { name: /check in/i });

    expect(button).not.toBeInTheDocument();
  });
});
