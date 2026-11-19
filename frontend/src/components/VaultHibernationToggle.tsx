import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';

/**
 * Hibernation state rendered on the vault card.
 *
 * `unknown` is used while the contract call that reads `is_hibernating` is
 * still in flight, or when the vault could not be loaded.
 */
export type HibernationStatus = 'active' | 'hibernating' | 'unknown';

/**
 * On-chain hibernation entry returned by the vault contract.
 *
 * Mirrors the fields used by `enter_hibernation` / `exit_hibernation`:
 * `started_at + duration_seconds` is when the TTL countdown resumes and
 * `remaining_ttl_at_entry` is the TTL that is restored on exit.
 */
export interface HibernationEntry {
  /** Ledger timestamp (seconds) at which hibernation started. */
  startedAt: number;
  /** Requested hibernation duration in seconds. */
  durationSeconds: number;
  /** TTL remaining (seconds) when hibernation started. */
  remainingTtlAtEntry: number;
}

/**
 * Contract surface used by the toggle.
 *
 * The dashboard supplies an adapter around the Soroban client so this
 * component stays transport agnostic and easy to test. Implementations must
 * forward the calls to the vault contract's `enter_hibernation` and
 * `exit_hibernation` entry points, and reject when the transaction fails.
 */
export interface VaultHibernationContract {
  enterHibernation(vaultId: string, caller: string, durationSeconds: number): Promise<void>;
  exitHibernation(vaultId: string, caller: string): Promise<void>;
}

/** Contract bounds for `duration_seconds` (see docs/openapi.yaml). */
export const MIN_HIBERNATION_SECONDS = 3_600;
export const MAX_HIBERNATION_SECONDS = 15_552_000;

/** Default request: seven days of hibernation. */
export const DEFAULT_HIBERNATION_SECONDS = 7 * 24 * 60 * 60;

export interface VaultHibernationToggleProps {
  vaultId: string;
  /** Connected wallet address; required for the owner-only contract calls. */
  owner?: string;
  status: HibernationStatus;
  /** Current entry when `status === 'hibernating'`. */
  entry?: HibernationEntry | null;
  contract: VaultHibernationContract;
  /** Requested duration when entering hibernation, in seconds. */
  durationSeconds?: number;
  onChanged?: (status: HibernationStatus) => void;
  className?: string;
}

/**
 * Formats a duration in seconds as a short human-readable string.
 *
 * Exported for reuse by the vault card and covered directly by unit tests.
 */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return '0 hours';
  }
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  const parts: string[] = [];
  if (days > 0) parts.push(`${days} ${days === 1 ? 'day' : 'days'}`);
  if (hours > 0) parts.push(`${hours} ${hours === 1 ? 'hour' : 'hours'}`);
  if (days === 0 && hours === 0 && minutes > 0) {
    parts.push(`${minutes} ${minutes === 1 ? 'minute' : 'minutes'}`);
  }
  return parts.join(' ') || 'less than a minute';
}

/**
 * Estimated instant at which the vault's TTL countdown resumes.
 *
 * Entering hibernation pauses the TTL; the contract restores
 * `remaining_ttl_at_entry` when hibernation ends. `null` means the resume
 * time cannot be estimated (for example, unknown state).
 */
export function estimateTtlResumeAt(
  status: HibernationStatus,
  entry: HibernationEntry | null | undefined,
): Date | null {
  if (status === 'hibernating' && entry) {
    return new Date((entry.startedAt + entry.durationSeconds) * 1000);
  }
  return null;
}

const STATUS_LABEL: Record<HibernationStatus, string> = {
  active: 'Active',
  hibernating: 'Hibernating',
  unknown: 'Status unknown',
};

const STATUS_COLOR: Record<HibernationStatus, string> = {
  active: '#2e7d32',
  hibernating: '#1565c0',
  unknown: '#757575',
};

/**
 * Vault hibernation toggle for the legacy dashboard (#1750).
 *
 * Shows the current hibernation state on the vault card and exposes a
 * single-click toggle that asks for confirmation before calling the vault
 * contract. The confirmation modal quotes the estimated TTL resumption time
 * and warns that hibernation pauses the TTL countdown but does **not**
 * prevent release when the vault balance is zero.
 */
export function VaultHibernationToggle({
  vaultId,
  owner,
  status,
  entry,
  contract,
  durationSeconds = DEFAULT_HIBERNATION_SECONDS,
  onChanged,
  className,
}: VaultHibernationToggleProps) {
  const [currentStatus, setCurrentStatus] = useState<HibernationStatus>(status);
  const [isModalOpen, setModalOpen] = useState(false);
  const [isPending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const titleId = useId();
  const descriptionId = useId();
  const cancelRef = useRef<HTMLButtonElement>(null);

  // Keep the rendered state in sync when the parent reloads the vault.
  useEffect(() => {
    setCurrentStatus(status);
  }, [status]);

  const isHibernating = currentStatus === 'hibernating';
  const canToggle = Boolean(owner) && currentStatus !== 'unknown' && !isPending;

  const resumeAt = estimateTtlResumeAt(currentStatus, entry);
  const nextResumeAt = new Date(Date.now() + durationSeconds * 1000);
  const requestedDuration = formatDuration(durationSeconds);

  const openModal = () => {
    setError(null);
    setModalOpen(true);
  };

  const closeModal = () => {
    if (isPending) {
      return;
    }
    setModalOpen(false);
    setError(null);
  };

  // Move focus into the dialog when it opens so keyboard users are not left
  // behind on the trigger button.
  useEffect(() => {
    if (isModalOpen) {
      cancelRef.current?.focus();
    }
  }, [isModalOpen]);

  const confirm = async () => {
    if (!owner || isPending) {
      return;
    }

    setPending(true);
    setError(null);
    try {
      if (isHibernating) {
        await contract.exitHibernation(vaultId, owner);
        setCurrentStatus('active');
        onChanged?.('active');
      } else {
        await contract.enterHibernation(vaultId, owner, durationSeconds);
        setCurrentStatus('hibernating');
        onChanged?.('hibernating');
      }
      setModalOpen(false);
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(message || 'The hibernation request failed. Please try again.');
    } finally {
      setPending(false);
    }
  };

  const handleDialogKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.stopPropagation();
      closeModal();
    }
  };

  return (
    <section
      className={className}
      data-testid="vault-hibernation-toggle"
      aria-labelledby={`${titleId}-heading`}
      style={{ display: 'flex', flexDirection: 'column', gap: 8 }}
    >
      <h3 id={`${titleId}-heading`} style={{ margin: 0, fontSize: 16 }}>
        Vault hibernation
      </h3>

      <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
        <span
          data-testid="vault-hibernation-status"
          role="status"
          aria-live="polite"
          aria-label={`Hibernation status: ${STATUS_LABEL[currentStatus]}`}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 6,
            fontWeight: 600,
            color: STATUS_COLOR[currentStatus],
          }}
        >
          <span
            aria-hidden="true"
            style={{
              width: 10,
              height: 10,
              borderRadius: '50%',
              backgroundColor: STATUS_COLOR[currentStatus],
              display: 'inline-block',
            }}
          />
          {STATUS_LABEL[currentStatus]}
        </span>

        {isHibernating && resumeAt ? (
          <span data-testid="vault-hibernation-resume-at">
            TTL resumes on{' '}
            <time dateTime={resumeAt.toISOString()}>
              {resumeAt.toLocaleString(undefined, {
                dateStyle: 'medium',
                timeStyle: 'short',
              })}
            </time>
          </span>
        ) : null}

        {isHibernating && entry ? (
          <span data-testid="vault-hibernation-restored-ttl">
            {formatDuration(entry.remainingTtlAtEntry)} of TTL will be restored on exit
          </span>
        ) : null}
      </div>

      <button
        type="button"
        onClick={openModal}
        disabled={!canToggle}
        title={canToggle ? undefined : 'Connect your wallet to manage hibernation'}
        data-testid="vault-hibernation-toggle-button"
      >
        {isHibernating ? 'End hibernation' : 'Hibernate vault'}
      </button>

      {isModalOpen ? (
        <div
          role="presentation"
          onClick={(event) => {
            if (event.target === event.currentTarget) {
              closeModal();
            }
          }}
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0, 0, 0, 0.45)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            padding: 16,
          }}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            aria-describedby={descriptionId}
            data-testid="vault-hibernation-dialog"
            onKeyDown={handleDialogKeyDown}
            style={{
              background: '#fff',
              color: '#111',
              borderRadius: 8,
              padding: 20,
              width: 'min(420px, 100%)',
              display: 'flex',
              flexDirection: 'column',
              gap: 12,
            }}
          >
            <h2 id={titleId} style={{ margin: 0, fontSize: 18 }}>
              {isHibernating ? 'End hibernation?' : 'Hibernate this vault?'}
            </h2>

            <div id={descriptionId} style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <p style={{ margin: 0 }}>
                Hibernation pauses the TTL countdown. It does not prevent release if the vault
                balance is zero.
              </p>
              {isHibernating ? (
                <p style={{ margin: 0 }} data-testid="vault-hibernation-dialog-resume">
                  The TTL countdown resumes immediately and restores{' '}
                  {entry ? formatDuration(entry.remainingTtlAtEntry) : 'the saved TTL'}.
                </p>
              ) : (
                <p style={{ margin: 0 }} data-testid="vault-hibernation-dialog-resume">
                  The TTL countdown will resume around{' '}
                  <time dateTime={nextResumeAt.toISOString()}>
                    {nextResumeAt.toLocaleString(undefined, {
                      dateStyle: 'medium',
                      timeStyle: 'short',
                    })}
                  </time>{' '}
                  ({requestedDuration} from now).
                </p>
              )}
            </div>

            {error ? (
              <p
                role="alert"
                data-testid="vault-hibernation-error"
                style={{ margin: 0, color: '#d32f2f' }}
              >
                {error}
              </p>
            ) : null}

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
              <button ref={cancelRef} type="button" onClick={closeModal} disabled={isPending}>
                Cancel
              </button>
              <button type="button" onClick={() => void confirm()} disabled={isPending}>
                {isPending
                  ? 'Submitting…'
                  : isHibernating
                    ? 'End hibernation'
                    : 'Hibernate vault'}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

export default VaultHibernationToggle;
