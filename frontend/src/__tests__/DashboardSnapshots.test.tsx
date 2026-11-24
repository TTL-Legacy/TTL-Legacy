/**
 * Snapshot tests for dashboard components — Issue #2083.
 *
 * Covers: VaultHealthStatus, TtlCountdown, PasskeyDeviceTypeChart,
 *         TokenAllowlistAdmin, and VaultHibernationToggle.
 *
 * Run in CI via `npm test` (Jest + ts-jest). Fail on unexpected diffs:
 *   npm test -- --ci   (--ci flag causes Jest to treat snapshot mismatches as failures)
 *
 * To update snapshots intentionally:
 *   npm test -- --updateSnapshot
 */

import React from 'react';
import { render } from '@testing-library/react';

import { VaultHealthStatus } from '../components/VaultHealthStatus';
import { TtlCountdown } from '../components/TtlCountdown';
import { PasskeyDeviceTypeChart } from '../components/PasskeyDeviceTypeChart';
import { TokenAllowlistAdmin } from '../components/TokenAllowlistAdmin';
import {
  VaultHibernationToggle,
  type HibernationEntry,
  type VaultHibernationContract,
} from '../components/VaultHibernationToggle';

// ── Helpers ────────────────────────────────────────────────────────────────

/** Fixed Unix timestamp used so TtlCountdown snapshots are deterministic. */
const FIXED_NOW_S = 1_700_000_000;

beforeEach(() => {
  jest.useFakeTimers();
  jest.setSystemTime(FIXED_NOW_S * 1000);
});

afterEach(() => {
  jest.useRealTimers();
});

function makeContract(
  overrides: Partial<VaultHibernationContract> = {},
): VaultHibernationContract {
  return {
    enterHibernation: jest.fn().mockResolvedValue(undefined),
    exitHibernation: jest.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

const HIBERNATION_ENTRY: HibernationEntry = {
  startedAt: FIXED_NOW_S - 3_600,
  durationSeconds: 604_800,
  remainingTtlAtEntry: 86_400,
};

// ── VaultHealthStatus ──────────────────────────────────────────────────────

describe('VaultHealthStatus snapshots', () => {
  it('renders healthy status with vault id', () => {
    const { container } = render(
      <VaultHealthStatus status="healthy" vaultId="vault-abc-1" />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders degraded status without vault id', () => {
    const { container } = render(<VaultHealthStatus status="degraded" />);
    expect(container).toMatchSnapshot();
  });

  it('renders down status', () => {
    const { container } = render(
      <VaultHealthStatus status="down" vaultId="vault-abc-2" />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders unknown status', () => {
    const { container } = render(<VaultHealthStatus status="unknown" />);
    expect(container).toMatchSnapshot();
  });
});

// ── TtlCountdown ───────────────────────────────────────────────────────────

describe('TtlCountdown snapshots', () => {
  it('renders ok state (time well above thresholds)', () => {
    const { container } = render(
      <TtlCountdown expiresAt={FIXED_NOW_S + 90_000} />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders warning state (within 24-hour threshold)', () => {
    const { container } = render(
      <TtlCountdown
        expiresAt={FIXED_NOW_S + 7_200}
        warningThreshold={86_400}
        criticalThreshold={3_600}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders critical state (within 1-hour threshold)', () => {
    const { container } = render(
      <TtlCountdown
        expiresAt={FIXED_NOW_S + 1_800}
        warningThreshold={86_400}
        criticalThreshold={3_600}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders expired state', () => {
    const { container } = render(
      <TtlCountdown expiresAt={FIXED_NOW_S - 1} />,
    );
    expect(container).toMatchSnapshot();
  });
});

// ── PasskeyDeviceTypeChart ─────────────────────────────────────────────────

describe('PasskeyDeviceTypeChart snapshots', () => {
  it('renders a balanced distribution', () => {
    const { container } = render(
      <PasskeyDeviceTypeChart
        checkIns={{ platform: 50, 'cross-platform': 30, hybrid: 20 }}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders with all check-ins in one category', () => {
    const { container } = render(
      <PasskeyDeviceTypeChart
        checkIns={{ platform: 100, 'cross-platform': 0, hybrid: 0 }}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders an empty state (all zeroes)', () => {
    const { container } = render(
      <PasskeyDeviceTypeChart
        checkIns={{ platform: 0, 'cross-platform': 0, hybrid: 0 }}
      />,
    );
    expect(container).toMatchSnapshot();
  });
});

// ── TokenAllowlistAdmin ────────────────────────────────────────────────────

describe('TokenAllowlistAdmin snapshots', () => {
  const TOKEN_A = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'; // 56 chars
  const TOKEN_B = 'CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB'; // 56 chars

  it('renders an empty allowlist for an admin', () => {
    const { container } = render(
      <TokenAllowlistAdmin
        tokens={[]}
        isAdmin={true}
        onAdd={jest.fn()}
        onRemove={jest.fn()}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders with tokens in the allowlist', () => {
    const { container } = render(
      <TokenAllowlistAdmin
        tokens={[TOKEN_A, TOKEN_B]}
        isAdmin={true}
        onAdd={jest.fn()}
        onRemove={jest.fn()}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders nothing for a non-admin', () => {
    const { container } = render(
      <TokenAllowlistAdmin
        tokens={[TOKEN_A]}
        isAdmin={false}
        onAdd={jest.fn()}
        onRemove={jest.fn()}
      />,
    );
    expect(container).toMatchSnapshot();
  });
});

// ── VaultHibernationToggle ─────────────────────────────────────────────────

describe('VaultHibernationToggle snapshots', () => {
  it('renders active state with owner connected', () => {
    const { container } = render(
      <VaultHibernationToggle
        vaultId="vault-snap-1"
        owner="GOWNER"
        status="active"
        contract={makeContract()}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders hibernating state with entry and owner', () => {
    const { container } = render(
      <VaultHibernationToggle
        vaultId="vault-snap-2"
        owner="GOWNER"
        status="hibernating"
        entry={HIBERNATION_ENTRY}
        contract={makeContract()}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders unknown state without owner (button disabled)', () => {
    const { container } = render(
      <VaultHibernationToggle
        vaultId="vault-snap-3"
        status="unknown"
        contract={makeContract()}
      />,
    );
    expect(container).toMatchSnapshot();
  });

  it('renders active state without owner (button disabled)', () => {
    const { container } = render(
      <VaultHibernationToggle
        vaultId="vault-snap-4"
        status="active"
        contract={makeContract()}
      />,
    );
    expect(container).toMatchSnapshot();
  });
});
