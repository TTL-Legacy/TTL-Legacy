import React, { useEffect, useState } from 'react';

export interface TtlCountdownProps {
  /** Unix timestamp (seconds) when the TTL expires */
  expiresAt: number;
  /** Seconds before expiry at which the countdown turns yellow (default: 86400 = 24 h) */
  warningThreshold?: number;
  /** Seconds before expiry at which the countdown turns red (default: 3600 = 1 h) */
  criticalThreshold?: number;
  className?: string;
}

type CountdownStatus = 'ok' | 'warning' | 'critical' | 'expired';

function getStatus(remaining: number, warning: number, critical: number): CountdownStatus {
  if (remaining <= 0) return 'expired';
  if (remaining <= critical) return 'critical';
  if (remaining <= warning) return 'warning';
  return 'ok';
}

const STATUS_COLORS: Record<CountdownStatus, string> = {
  ok: '#2e7d32',
  warning: '#ed6c02',
  critical: '#d32f2f',
  expired: '#757575',
};

function formatDuration(seconds: number): string {
  if (seconds <= 0) return 'Expired';
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  return `${m}m ${s}s`;
}

/**
 * Displays a live countdown to the next required check-in (#1550).
 *
 * Color changes to yellow when approaching expiry and red when critical.
 * Renders as an accessible `<time>` element with `role="timer"`.
 */
export function TtlCountdown({
  expiresAt,
  warningThreshold = 86400,
  criticalThreshold = 3600,
  className,
}: TtlCountdownProps) {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    const id = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(id);
  }, []);

  const remaining = expiresAt - now;
  const status = getStatus(remaining, warningThreshold, criticalThreshold);
  const color = STATUS_COLORS[status];
  const label = formatDuration(Math.max(0, remaining));

  return (
    <time
      className={className}
      role="timer"
      aria-label={`Time until check-in required: ${label}`}
      aria-live="off"
      dateTime={new Date(expiresAt * 1000).toISOString()}
      data-testid="ttl-countdown"
      style={{ color, fontVariantNumeric: 'tabular-nums' }}
    >
      {label}
    </time>
  );
}

export default TtlCountdown;
