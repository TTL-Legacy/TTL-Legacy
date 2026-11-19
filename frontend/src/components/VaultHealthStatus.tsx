import React from 'react';

export type VaultHealth = 'healthy' | 'degraded' | 'down' | 'unknown';

const LABELS: Record<VaultHealth, string> = {
  healthy: 'Vault healthy',
  degraded: 'Vault degraded',
  down: 'Vault down',
  unknown: 'Vault status unknown',
};

const COLORS: Record<VaultHealth, string> = {
  healthy: '#2e7d32',
  degraded: '#ed6c02',
  down: '#d32f2f',
  unknown: '#757575',
};

export interface VaultHealthStatusProps {
  status: VaultHealth;
  vaultId?: string;
  className?: string;
}

/**
 * Accessible vault-health status indicator for the legacy dashboard (#1298).
 *
 * Renders a colored dot plus a human-readable, screen-reader-announced label.
 * `role="status"` + `aria-live="polite"` means state changes are announced
 * without moving focus.
 */
export function VaultHealthStatus({ status, vaultId, className }: VaultHealthStatusProps) {
  const label = vaultId ? `${LABELS[status]} (${vaultId})` : LABELS[status];
  return (
    <span
      className={className}
      role="status"
      aria-live="polite"
      aria-label={label}
      data-testid="vault-health-status"
      style={{ display: 'inline-flex', alignItems: 'center', gap: 6, color: COLORS[status] }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 10,
          height: 10,
          borderRadius: '50%',
          backgroundColor: COLORS[status],
          display: 'inline-block',
        }}
      />
      {label}
    </span>
  );
}

export default VaultHealthStatus;
