import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import { VaultDetailPage } from '../components/VaultDetailPage';

// Mock useParams hook
jest.mock('react-router-dom', () => ({
  useParams: () => ({ vaultId: 'vault-123' }),
}));

// Mock vault API
const mockFetchVaultDetails = jest.fn();
const mockFetchVaultEvents = jest.fn();
jest.mock('../api/vault', () => ({
  fetchVaultDetails: mockFetchVaultDetails,
  fetchVaultEvents: mockFetchVaultEvents,
}));

describe('VaultDetailPage', () => {
  const mockVaultDetails = {
    id: 'vault-123',
    owner: 'GOWNER123456789',
    balance: '5000.00',
    checkInInterval: 30,
    beneficiaries: [
      {
        address: 'GBENEFICIARY1',
        allocation: 5000, // 50%
      },
      {
        address: 'GBENEFICIARY2',
        allocation: 5000, // 50%
      },
    ],
    createdAt: '2024-01-01T00:00:00Z',
    lastCheckIn: '2024-09-20T00:00:00Z',
  };

  const mockVaultEvents = [
    {
      id: 'event-1',
      type: 'check_in',
      timestamp: '2024-09-20T12:00:00Z',
      txHash: 'abc123',
    },
    {
      id: 'event-2',
      type: 'deposit',
      amount: '1000.00',
      timestamp: '2024-09-15T10:00:00Z',
      txHash: 'def456',
    },
  ];

  beforeEach(() => {
    jest.clearAllMocks();
    mockFetchVaultDetails.mockResolvedValue(mockVaultDetails);
    mockFetchVaultEvents.mockResolvedValue(mockVaultEvents);
  });

  it('renders vault detail page', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/vault details/i)).toBeInTheDocument();
    });
  });

  it('displays vault balance', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/5000/)).toBeInTheDocument();
    });
  });

  it('displays check-in interval', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/30/)).toBeInTheDocument();
    });
  });

  it('displays all beneficiaries with their allocations', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/GBENEFICIARY1/)).toBeInTheDocument();
      expect(screen.getByText(/GBENEFICIARY2/)).toBeInTheDocument();
    });
  });

  it('shows BPS allocations for beneficiaries', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/50%/)).toBeInTheDocument();
    });
  });

  it('displays recent events', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/recent events/i)).toBeInTheDocument();
    });
  });

  it('shows check-in events in event history', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/check.in/i)).toBeInTheDocument();
    });
  });

  it('shows deposit events in event history', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/deposit/i)).toBeInTheDocument();
    });
  });

  it('displays transaction hashes for events', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/abc123/)).toBeInTheDocument();
      expect(screen.getByText(/def456/)).toBeInTheDocument();
    });
  });

  it('displays owner address', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/GOWNER123456789/)).toBeInTheDocument();
    });
  });

  it('shows creation date', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/2024-01-01/)).toBeInTheDocument();
    });
  });

  it('shows last check-in timestamp', async () => {
    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/2024-09-20/)).toBeInTheDocument();
    });
  });

  it('displays loading state while fetching data', () => {
    mockFetchVaultDetails.mockImplementation(
      () => new Promise(resolve => setTimeout(() => resolve(mockVaultDetails), 100))
    );

    render(<VaultDetailPage />);

    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it('handles vault details fetch error', async () => {
    mockFetchVaultDetails.mockRejectedValue(new Error('Failed to fetch vault'));

    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/error loading vault/i)).toBeInTheDocument();
    });
  });

  it('handles vault events fetch error gracefully', async () => {
    mockFetchVaultEvents.mockRejectedValue(new Error('Failed to fetch events'));

    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/vault details/i)).toBeInTheDocument();
    });

    expect(screen.getByText(/vault details/i)).toBeInTheDocument();
  });

  it('displays empty state when no beneficiaries', async () => {
    mockFetchVaultDetails.mockResolvedValue({
      ...mockVaultDetails,
      beneficiaries: [],
    });

    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/no beneficiaries/i)).toBeInTheDocument();
    });
  });

  it('displays empty state when no events', async () => {
    mockFetchVaultEvents.mockResolvedValue([]);

    render(<VaultDetailPage />);

    await waitFor(() => {
      expect(screen.getByText(/no events/i)).toBeInTheDocument();
    });
  });
});
