import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CreateVaultForm } from '../components/CreateVaultForm';

// Mock wallet hook
jest.mock('../hooks/useWallet', () => ({
  useWallet: jest.fn(() => ({
    isConnected: true,
    address: 'GTEST123456789',
    signTransaction: jest.fn().mockResolvedValue({ hash: 'tx123' }),
  })),
}));

// Mock vault API
const mockCreateVault = jest.fn();
jest.mock('../api/vault', () => ({
  createVault: mockCreateVault,
}));

describe('CreateVaultForm', () => {
  const mockOnSuccess = jest.fn();

  beforeEach(() => {
    jest.clearAllMocks();
    mockCreateVault.mockResolvedValue({ success: true, vaultId: 'vault-123' });
  });

  it('renders the vault creation form', () => {
    render(<CreateVaultForm onSuccess={mockOnSuccess} />);
    expect(screen.getByText(/create vault/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/check-in interval/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/deposit/i)).toBeInTheDocument();
  });

  it('accepts check-in interval input', async () => {
    const user = userEvent.setup();
    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    const intervalInput = screen.getByLabelText(/check-in interval/i) as HTMLInputElement;
    await user.clear(intervalInput);
    await user.type(intervalInput, '30');

    expect(intervalInput.value).toBe('30');
  });

  it('accepts deposit amount input', async () => {
    const user = userEvent.setup();
    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    const depositInput = screen.getByLabelText(/deposit/i) as HTMLInputElement;
    await user.clear(depositInput);
    await user.type(depositInput, '1000');

    expect(depositInput.value).toBe('1000');
  });

  it('allows adding beneficiaries with BPS allocation', async () => {
    const user = userEvent.setup();
    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    const addBeneficiaryBtn = screen.getByText(/add beneficiary/i);
    await user.click(addBeneficiaryBtn);

    expect(screen.getByLabelText(/beneficiary address/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/allocation \(bps\)/i)).toBeInTheDocument();
  });

  it('validates that BPS allocations sum to 10000', async () => {
    const user = userEvent.setup();
    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    // Add first beneficiary with 5000 BPS
    const addBtn = screen.getByText(/add beneficiary/i);
    await user.click(addBtn);

    const bpsInputs = screen.getAllByLabelText(/allocation \(bps\)/i);
    await user.clear(bpsInputs[0]);
    await user.type(bpsInputs[0], '5000');

    // Submit should show validation error
    const submitBtn = screen.getByRole('button', { name: /create vault/i });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(screen.getByText(/allocations must sum to 100%/i)).toBeInTheDocument();
    });
  });

  it('allows form submission when BPS sums to 10000', async () => {
    const user = userEvent.setup();
    mockCreateVault.mockResolvedValue({ success: true, vaultId: 'vault-456' });

    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    // Fill in basic fields
    const intervalInput = screen.getByLabelText(/check-in interval/i);
    const depositInput = screen.getByLabelText(/deposit/i);

    await user.clear(intervalInput);
    await user.type(intervalInput, '30');
    await user.clear(depositInput);
    await user.type(depositInput, '1000');

    // Add single beneficiary with 10000 BPS (100%)
    const addBtn = screen.getByText(/add beneficiary/i);
    await user.click(addBtn);

    const addressInputs = screen.getAllByLabelText(/beneficiary address/i);
    const bpsInputs = screen.getAllByLabelText(/allocation \(bps\)/i);

    await user.type(addressInputs[0], 'GBENEFICIARY111');
    await user.clear(bpsInputs[0]);
    await user.type(bpsInputs[0], '10000');

    const submitBtn = screen.getByRole('button', { name: /create vault/i });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(mockCreateVault).toHaveBeenCalled();
    });
  });

  it('shows success message when vault is created', async () => {
    const user = userEvent.setup();
    mockCreateVault.mockResolvedValue({ success: true, vaultId: 'vault-789' });

    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    // Fill minimum required fields
    const intervalInput = screen.getByLabelText(/check-in interval/i);
    const depositInput = screen.getByLabelText(/deposit/i);

    await user.clear(intervalInput);
    await user.type(intervalInput, '30');
    await user.clear(depositInput);
    await user.type(depositInput, '1000');

    const addBtn = screen.getByText(/add beneficiary/i);
    await user.click(addBtn);

    const addressInputs = screen.getAllByLabelText(/beneficiary address/i);
    const bpsInputs = screen.getAllByLabelText(/allocation \(bps\)/i);

    await user.type(addressInputs[0], 'GBENEFICIARY222');
    await user.clear(bpsInputs[0]);
    await user.type(bpsInputs[0], '10000');

    const submitBtn = screen.getByRole('button', { name: /create vault/i });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(screen.getByText(/vault created successfully/i)).toBeInTheDocument();
    });
  });

  it('calls onSuccess callback after vault creation', async () => {
    const user = userEvent.setup();
    mockCreateVault.mockResolvedValue({ success: true, vaultId: 'vault-success' });

    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    const intervalInput = screen.getByLabelText(/check-in interval/i);
    const depositInput = screen.getByLabelText(/deposit/i);

    await user.clear(intervalInput);
    await user.type(intervalInput, '30');
    await user.clear(depositInput);
    await user.type(depositInput, '1000');

    const addBtn = screen.getByText(/add beneficiary/i);
    await user.click(addBtn);

    const addressInputs = screen.getAllByLabelText(/beneficiary address/i);
    const bpsInputs = screen.getAllByLabelText(/allocation \(bps\)/i);

    await user.type(addressInputs[0], 'GBENEFICIARY333');
    await user.clear(bpsInputs[0]);
    await user.type(bpsInputs[0], '10000');

    const submitBtn = screen.getByRole('button', { name: /create vault/i });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(mockOnSuccess).toHaveBeenCalled();
    });
  });

  it('shows error message on vault creation failure', async () => {
    const user = userEvent.setup();
    mockCreateVault.mockRejectedValue(new Error('Network error'));

    render(<CreateVaultForm onSuccess={mockOnSuccess} />);

    const intervalInput = screen.getByLabelText(/check-in interval/i);
    const depositInput = screen.getByLabelText(/deposit/i);

    await user.clear(intervalInput);
    await user.type(intervalInput, '30');
    await user.clear(depositInput);
    await user.type(depositInput, '1000');

    const addBtn = screen.getByText(/add beneficiary/i);
    await user.click(addBtn);

    const addressInputs = screen.getAllByLabelText(/beneficiary address/i);
    const bpsInputs = screen.getAllByLabelText(/allocation \(bps\)/i);

    await user.type(addressInputs[0], 'GBENEFICIARY444');
    await user.clear(bpsInputs[0]);
    await user.type(bpsInputs[0], '10000');

    const submitBtn = screen.getByRole('button', { name: /create vault/i });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(screen.getByText(/error creating vault/i)).toBeInTheDocument();
    });
  });
});
