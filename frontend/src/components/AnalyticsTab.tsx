import React from 'react';
import {
  DeviceTypeCheckIns,
  PasskeyDeviceTypeChart,
} from './PasskeyDeviceTypeChart';

export interface AnalyticsTabProps {
  checkInsByDeviceType: DeviceTypeCheckIns;
}

/**
 * Analytics tab with check-ins broken down by passkey device type (#1762).
 */
export function AnalyticsTab({ checkInsByDeviceType }: AnalyticsTabProps) {
  return (
    <section role="tabpanel" aria-label="Analytics" data-testid="analytics-tab">
      <h2>Analytics</h2>
      <PasskeyDeviceTypeChart checkIns={checkInsByDeviceType} />
    </section>
  );
}
