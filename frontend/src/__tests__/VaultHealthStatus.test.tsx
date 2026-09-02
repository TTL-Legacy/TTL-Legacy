import { render, screen } from '@testing-library/react';
import { VaultHealthStatus } from '../components/VaultHealthStatus';

describe('VaultHealthStatus', () => {
  it('renders a healthy status with the vault id in the accessible label', () => {
    render(<VaultHealthStatus status="healthy" vaultId="vault-1" />);
    const el = screen.getByRole('status');
    expect(el).toHaveTextContent('Vault healthy (vault-1)');
    expect(el).toHaveAttribute('aria-label', 'Vault healthy (vault-1)');
  });

  it('reflects a down status', () => {
    render(<VaultHealthStatus status="down" />);
    expect(screen.getByLabelText('Vault down')).toBeInTheDocument();
  });

  it('reflects a degraded status', () => {
    render(<VaultHealthStatus status="degraded" vaultId="v2" />);
    expect(screen.getByText('Vault degraded (v2)')).toBeInTheDocument();
  });

  it('defaults to unknown when no status detail is provided', () => {
    render(<VaultHealthStatus status="unknown" />);
    expect(screen.getByLabelText('Vault status unknown')).toBeInTheDocument();
  });
});
