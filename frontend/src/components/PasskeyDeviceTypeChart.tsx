import React from 'react';

export interface DeviceTypeCheckIns {
  platform: number;
  'cross-platform': number;
  hybrid: number;
}

export interface PasskeyDeviceTypeChartProps {
  checkIns: DeviceTypeCheckIns;
}

const SEGMENTS: { key: keyof DeviceTypeCheckIns; label: string; color: string }[] = [
  { key: 'platform', label: 'Platform', color: '#1565c0' },
  { key: 'cross-platform', label: 'Cross-platform', color: '#6a1b9a' },
  { key: 'hybrid', label: 'Hybrid', color: '#00838f' },
];

/**
 * Stacked bar of check-ins by passkey device type (#1762).
 */
export function PasskeyDeviceTypeChart({ checkIns }: PasskeyDeviceTypeChartProps) {
  const total = SEGMENTS.reduce((sum, segment) => sum + checkIns[segment.key], 0);
  const summary = SEGMENTS.map(
    (segment) => `${segment.label} ${checkIns[segment.key]}`
  ).join(', ');

  return (
    <figure data-testid="passkey-device-type-chart">
      <figcaption>Check-ins by device type</figcaption>
      <div
        role="img"
        aria-label={summary}
        style={{
          display: 'flex',
          width: '100%',
          height: 24,
          background: '#e0e0e0',
        }}
      >
        {SEGMENTS.map((segment) => {
          const count = checkIns[segment.key];
          const width = total === 0 ? 0 : (count / total) * 100;
          return (
            <div
              key={segment.key}
              data-testid={`device-type-${segment.key}`}
              title={`${segment.label}: ${count}`}
              style={{ width: `${width}%`, background: segment.color }}
            />
          );
        })}
      </div>
      <ul>
        {SEGMENTS.map((segment) => (
          <li key={segment.key}>
            {segment.label}: {checkIns[segment.key]}
          </li>
        ))}
      </ul>
    </figure>
  );
}
