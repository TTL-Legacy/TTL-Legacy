import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { apiClient } from '../api/client';

export interface UpgradeProposal {
  id: string;
  title?: string;
  description?: string;
  proposer?: string;
  contractAddress?: string;
  newWasmHash?: string;
  proposedAt?: string | number;
  timelockEndsAt?: string | number;
  status?: string;
}

export interface UpgradeProposalDashboardProps {
  /** Connected wallet address (e.g. from wallet context). */
  walletAddress?: string | null;
  /** Contract admin address. If omitted, it is fetched from the API client. */
  adminAddress?: string | null;
  /** Optional override for the proposal list fetcher (useful for tests). */
  fetchProposals?: () => Promise<UpgradeProposal[]>;
  /** Optional override for the approve handler. */
  onApprove?: (proposal: UpgradeProposal) => Promise<void> | void;
  /** Optional override for the reject handler. */
  onReject?: (proposal: UpgradeProposal) => Promise<void> | void;
}

interface ProposalActionState {
  pending: boolean;
  error?: string;
}

const toMillis = (value?: string | number | null): number | null => {
  if (value === null || value === undefined) return null;
  if (typeof value === 'number') {
    // Heuristic: treat values below 1e12 as seconds since epoch.
    return value < 1e12 ? value * 1000 : value;
  }
  const parsed = Date.parse(value);
  if (Number.isNaN(parsed)) {
    const numeric = Number(value);
    if (!Number.isNaN(numeric)) {
      return numeric < 1e12 ? numeric * 1000 : numeric;
    }
    return null;
  }
  return parsed;
};

const formatDuration = (ms: number): string => {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const days = Math.floor(totalSeconds / 86400);
  const hours = Math.floor((totalSeconds % 86400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  if (days > 0) {
    return `${days}d ${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
  }
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
};

const normalizeAddress = (address?: string | null): string =>
  (address ?? '').trim().toLowerCase();

const defaultFetchProposals = async (): Promise<UpgradeProposal[]> => {
  const client = apiClient as unknown as {
    getUpgradeProposals?: () => Promise<UpgradeProposal[]>;
    getPendingUpgradeProposals?: () => Promise<UpgradeProposal[]>;
  };
  if (typeof client.getUpgradeProposals === 'function') {
    return client.getUpgradeProposals();
  }
  if (typeof client.getPendingUpgradeProposals === 'function') {
    return client.getPendingUpgradeProposals();
  }
  throw new Error('Upgrade proposal API is not available');
};

const defaultFetchAdminAddress = async (): Promise<string | null> => {
  const client = apiClient as unknown as {
    getContractAdmin?: () => Promise<string | null>;
    getAdminAddress?: () => Promise<string | null>;
  };
  if (typeof client.getContractAdmin === 'function') {
    return client.getContractAdmin();
  }
  if (typeof client.getAdminAddress === 'function') {
    return client.getAdminAddress();
  }
  return null;
};

const defaultApprove = async (proposal: UpgradeProposal): Promise<void> => {
  const client = apiClient as unknown as {
    approveUpgradeProposal?: (id: string) => Promise<void>;
  };
  if (typeof client.approveUpgradeProposal !== 'function') {
    throw new Error('Approve API is not available');
  }
  await client.approveUpgradeProposal(proposal.id);
};

const defaultReject = async (proposal: UpgradeProposal): Promise<void> => {
  const client = apiClient as unknown as {
    rejectUpgradeProposal?: (id: string) => Promise<void>;
  };
  if (typeof client.rejectUpgradeProposal !== 'function') {
    throw new Error('Reject API is not available');
  }
  await client.rejectUpgradeProposal(proposal.id);
};

const UpgradeProposalDashboard: React.FC<UpgradeProposalDashboardProps> = ({
  walletAddress,
  adminAddress,
  fetchProposals,
  onApprove,
  onReject,
}) => {
  const { t } = useTranslation('upgradeProposal');

  const [proposals, setProposals] = useState<UpgradeProposal[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [resolvedAdmin, setResolvedAdmin] = useState<string | null>(
    adminAddress ?? null,
  );
  const [now, setNow] = useState<number>(() => Date.now());
  const [actionStates, setActionStates] = useState<
    Record<string, ProposalActionState>
  >({});
  const mountedRef = useRef<boolean>(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Live countdown ticker.
  useEffect(() => {
    const interval = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => window.clearInterval(interval);
  }, []);

  const loadProposals = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const fetcher = fetchProposals ?? defaultFetchProposals;
      const result = await fetcher();
      if (!mountedRef.current) return;
      setProposals(Array.isArray(result) ? result : []);
    } catch (err) {
      if (!mountedRef.current) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (mountedRef.current) {
        setLoading(false);
      }
    }
  }, [fetchProposals]);

  useEffect(() => {
    void loadProposals();
  }, [loadProposals]);

  useEffect(() => {
    if (adminAddress !== undefined && adminAddress !== null) {
      setResolvedAdmin(adminAddress);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const fetched = await defaultFetchAdminAddress();
        if (!cancelled && mountedRef.current) {
          setResolvedAdmin(fetched);
        }
      } catch {
        if (!cancelled && mountedRef.current) {
          setResolvedAdmin(null);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [adminAddress]);

  const isAdmin = useMemo(() => {
    const wallet = normalizeAddress(walletAddress);
    const admin = normalizeAddress(resolvedAdmin);
    return wallet.length > 0 && admin.length > 0 && wallet === admin;
  }, [walletAddress, resolvedAdmin]);

  const handleAction = useCallback(
    async (proposal: UpgradeProposal, action: 'approve' | 'reject') => {
      setActionStates((prev) => ({
        ...prev,
        [proposal.id]: { pending: true },
      }));
      try {
        if (action === 'approve') {
          await (onApprove ?? defaultApprove)(proposal);
        } else {
          await (onReject ?? defaultReject)(proposal);
        }
        if (!mountedRef.current) return;
        setActionStates((prev) => {
          const next = { ...prev };
          delete next[proposal.id];
          return next;
        });
        await loadProposals();
      } catch (err) {
        if (!mountedRef.current) return;
        setActionStates((prev) => ({
          ...prev,
          [proposal.id]: {
            pending: false,
            error: err instanceof Error ? err.message : String(err),
          },
        }));
      }
    },
    [onApprove, onReject, loadProposals],
  );

  const renderCountdown = (proposal: UpgradeProposal) => {
    const endsAt = toMillis(proposal.timelockEndsAt);
    if (endsAt === null) {
      return (
        <span className="upgrade-proposal__countdown upgrade-proposal__countdown--unknown">
          {t('timelock.unknown')}
        </span>
      );
    }
    const remaining = endsAt - now;
    if (remaining <= 0) {
      return (
        <span className="upgrade-proposal__countdown upgrade-proposal__countdown--ready">
          {t('timelock.ready')}
        </span>
      );
    }
    return (
      <span
        className="upgrade-proposal__countdown"
        aria-live="polite"
        data-testid={`countdown-${proposal.id}`}
      >
        {t('timelock.remaining', { time: formatDuration(remaining) })}
      </span>
    );
  };

  return (
    <section
      className="upgrade-proposal-dashboard"
      aria-labelledby="upgrade-proposal-dashboard-title"
    >
      <header className="upgrade-proposal-dashboard__header">
        <h2 id="upgrade-proposal-dashboard-title">
          {t('title')}
        </h2>
        <button
          type="button"
          className="upgrade-proposal-dashboard__refresh"
          onClick={() => void loadProposals()}
          disabled={loading}
        >
          {t('refresh')}
        </button>
      </header>

      {loading && (
        <div
          className="upgrade-proposal-dashboard__loading"
          role="status"
          aria-live="polite"
        >
          {t('loading')}
        </div>
      )}

      {!loading && error && (
        <div
          className="upgrade-proposal-dashboard__error"
          role="alert"
        >
          <span>{t('error', { message: error })}</span>
          <button type="button" onClick={() => void loadProposals()}>
            {t('retry')}
          </button>
        </div>
      )}

      {!loading && !error && proposals.length === 0 && (
        <div className="upgrade-proposal-dashboard__empty">
          {t('empty')}
        </div>
      )}

      {!loading && !error && proposals.length > 0 && (
        <ul className="upgrade-proposal-dashboard__list">
          {proposals.map((proposal) => {
            const actionState = actionStates[proposal.id];
            const pending = actionState?.pending ?? false;
            return (
              <li
                key={proposal.id}
                className="upgrade-proposal"
                data-testid={`proposal-${proposal.id}`}
              >
                <div className="upgrade-proposal__info">
                  <h3 className="upgrade-proposal__title">
                    {proposal.title || t('proposalFallbackTitle', { id: proposal.id })}
                  </h3>
                  {proposal.description && (
                    <p className="upgrade-proposal__description">
                      {proposal.description}
                    </p>
                  )}
                  {proposal.newWasmHash && (
                    <p className="upgrade-proposal__hash">
                      {t('newWasmHash', { hash: proposal.newWasmHash })}
                    </p>
                  )}
                  {proposal.proposer && (
                    <p className="upgrade-proposal__proposer">
                      {t('proposer', { address: proposal.proposer })}
                    </p>
                  )}
                  <p className="upgrade-proposal__timelock">
                    {t('timelock.label')}: {renderCountdown(proposal)}
                  </p>
                </div>

                {isAdmin && (
                  <div className="upgrade-proposal__actions">
                    <button
                      type="button"
                      className="upgrade-proposal__approve"
                      onClick={() => void handleAction(proposal, 'approve')}
                      disabled={pending}
                    >
                      {pending ? t('actions.processing') : t('actions.approve')}
                    </button>
                    <button
                      type="button"
                      className="upgrade-proposal__reject"
                      onClick={() => void handleAction(proposal, 'reject')}
                      disabled={pending}
                    >
                      {pending ? t('actions.processing') : t('actions.reject')}
                    </button>
                  </div>
                )}

                {actionState?.error && (
                  <div
                    className="upgrade-proposal__action-error"
                    role="alert"
                  >
                    {t('actionError', { message: actionState.error })}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
};

export default UpgradeProposalDashboard;