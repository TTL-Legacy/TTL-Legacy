import React, { useCallback, useEffect, useState } from 'react';
import { apiClient, type DrStatusResponse, type RestoreVerificationStatus } from '../api/client';

export interface DrStatusWidgetProps {
  /** Only admins see the DR status widget; non-admins get nothing rendered. */
  isAdmin: boolean;
  /** Optional admin bearer token forwarded to GET /admin/dr-status. */
  adminToken?: string;
  /** Optional custom fetcher for testing or custom clients. */
  fetchStatus?: () => Promise<DrStatusResponse>;
  className?: string;
}

const VERIFICATION_LABELS: Record<RestoreVerificationStatus, string> = {
  passed: 'Passed',
  failed: 'Failed',
  unknown: 'Unknown',
};

function formatTimestamp(iso: string | null | undefined): string {
  if (!iso) {
    return 'Never';
  }
  const parsed = new Date(iso);
  if (Number.isNaN(parsed.getTime())) {
    return iso;
  }
  return parsed.toISOString();
}

/**
 * Admin dashboard widget displaying Disaster Recovery (DR) status from
 * `GET /admin/dr-status`: last successful backup timestamp, last restore
 * verification result, and any active DR alerts.
 */
export function DrStatusWidget({
  isAdmin,
  adminToken,
  fetchStatus,
  className,
}: DrStatusWidgetProps) {
  const [status, setStatus] = useState<DrStatusResponse | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = fetchStatus
        ? await fetchStatus()
        : await apiClient.getDrStatus(adminToken);
      setStatus(data);
    } catch (err) {
      const message =
        err instanceof Error && err.message
          ? err.message
          : 'Failed to load disaster recovery status.';
      setError(message);
    } finally {
      setLoading(false);
    }
  }, [adminToken, fetchStatus]);

  useEffect(() => {
    if (!isAdmin) {
      return;
    }
    void loadStatus();
  }, [isAdmin, loadStatus]);

  if (!isAdmin) {
    return null;
  }

  return (
    <section
      className={className}
      aria-labelledby="dr-status-widget-heading"
      data-testid="dr-status-widget"
    >
      <div>
        <h2 id="dr-status-widget-heading">Disaster Recovery Status</h2>
        <button
          type="button"
          onClick={() => void loadStatus()}
          disabled={loading}
          aria-label="Refresh disaster recovery status"
        >
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {loading && !status && (
        <p role="status" data-testid="dr-status-loading">
          Loading disaster recovery status…
        </p>
      )}

      {error && (
        <p role="alert" data-testid="dr-status-error">
          {error}
        </p>
      )}

      {status && (
        <div>
          <dl>
            <div>
              <dt>Last Successful Backup</dt>
              <dd data-testid="dr-last-backup">
                {status.last_successful_backup_at
                  ? formatTimestamp(status.last_successful_backup_at)
                  : 'No successful backup recorded'}
              </dd>
            </div>

            <div>
              <dt>Last Restore Verification</dt>
              <dd data-testid="dr-restore-verification">
                <span data-status={status.last_restore_verification.status}>
                  {VERIFICATION_LABELS[status.last_restore_verification.status]}
                </span>
                {status.last_restore_verification.verified_at && (
                  <span>
                    {' '}
                    ({formatTimestamp(status.last_restore_verification.verified_at)})
                  </span>
                )}
                {status.last_restore_verification.backup_prefix && (
                  <div>
                    Prefix: <code>{status.last_restore_verification.backup_prefix}</code>
                  </div>
                )}
                {status.last_restore_verification.details && (
                  <div>{status.last_restore_verification.details}</div>
                )}
              </dd>
            </div>
          </dl>

          <div>
            <h3>Active DR Alerts</h3>
            {status.active_alerts.length === 0 ? (
              <p data-testid="dr-alerts-empty">No active DR alerts.</p>
            ) : (
              <ul aria-label="Active DR alerts" data-testid="dr-alerts-list">
                {status.active_alerts.map((alert) => (
                  <li key={alert.id} data-severity={alert.severity}>
                    <strong>[{alert.severity.toUpperCase()}]</strong> {alert.message}{' '}
                    <span>({formatTimestamp(alert.triggered_at)})</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

export default DrStatusWidget;
