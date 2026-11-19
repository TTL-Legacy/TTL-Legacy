import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import {
  DEFAULT_HIBERNATION_SECONDS,
  VaultHibernationToggle,
  estimateTtlResumeAt,
  formatDuration,
  type HibernationEntry,
  type VaultHibernationContract,
} from '../components/VaultHibernationToggle';

const VAULT_ID = '42';
const OWNER = 'GOWNERADDRESS';

const ENTRY: HibernationEntry = {
  startedAt: 1_700_000_000,
  durationSeconds: 604_800,
  remainingTtlAtEntry: 86_400,
};

function makeContract(overrides: Partial<VaultHibernationContract> = {}): jest.Mocked<VaultHibernationContract> {
  return {
    enterHibernation: jest.fn().mockResolvedValue(undefined),
    exitHibernation: jest.fn().mockResolvedValue(undefined),
    ...overrides,
  } as jest.Mocked<VaultHibernationContract>;
}

describe('formatDuration', () => {
  it('formats days and hours', () => {
    expect(formatDuration(86_400 + 3_600)).toBe('1 day 1 hour');
    expect(formatDuration(604_800)).toBe('7 days');
  });

  it('falls back to minutes for short durations', () => {
    expect(formatDuration(1_800)).toBe('30 minutes');
  });

  it('handles non-positive values', () => {
    expect(formatDuration(0)).toBe('0 hours');
    expect(formatDuration(Number.NaN)).toBe('0 hours');
  });
});

describe('estimateTtlResumeAt', () => {
  it('adds the duration to the hibernation start', () => {
    expect(estimateTtlResumeAt('hibernating', ENTRY)?.toISOString()).toBe(
      new Date((ENTRY.startedAt + ENTRY.durationSeconds) * 1_000).toISOString(),
    );
  });

  it('returns null for active and unknown vaults', () => {
    expect(estimateTtlResumeAt('active', ENTRY)).toBeNull();
    expect(estimateTtlResumeAt('unknown', null)).toBeNull();
    expect(estimateTtlResumeAt('hibernating', null)).toBeNull();
  });
});

describe('VaultHibernationToggle', () => {
  beforeEach(() => {
    jest.useFakeTimers();
    jest.setSystemTime(new Date('2026-01-01T00:00:00.000Z'));
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it('shows the active state and the enter action', () => {
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="active"
        contract={makeContract()}
      />,
    );

    expect(screen.getByTestId('vault-hibernation-status')).toHaveTextContent('Active');
    expect(screen.getByTestId('vault-hibernation-toggle-button')).toHaveTextContent(
      'Hibernate vault',
    );
    expect(screen.queryByTestId('vault-hibernation-resume-at')).not.toBeInTheDocument();
  });

  it('shows the hibernating state with the estimated TTL resume time', () => {
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="hibernating"
        entry={ENTRY}
        contract={makeContract()}
      />,
    );

    expect(screen.getByTestId('vault-hibernation-status')).toHaveTextContent('Hibernating');
    expect(screen.getByTestId('vault-hibernation-toggle-button')).toHaveTextContent(
      'End hibernation',
    );
    const resumeTime = screen
      .getByTestId('vault-hibernation-resume-at')
      .querySelector('time');
    expect(resumeTime).toHaveAttribute(
      'datetime',
      new Date((ENTRY.startedAt + ENTRY.durationSeconds) * 1_000).toISOString(),
    );
    expect(screen.getByTestId('vault-hibernation-restored-ttl')).toHaveTextContent(
      '1 day of TTL will be restored on exit',
    );
  });

  it('opens a confirmation modal warning that hibernation does not stop release', () => {
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="active"
        contract={makeContract()}
      />,
    );

    fireEvent.click(screen.getByTestId('vault-hibernation-toggle-button'));

    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByRole('heading', { name: 'Hibernate this vault?' })).toBeVisible();
    expect(
      within(dialog).getByText(/does not prevent release if the vault balance is zero/i),
    ).toBeVisible();

    // Estimated resumption is now + the requested duration.
    const expected = new Date(
      Date.now() + DEFAULT_HIBERNATION_SECONDS * 1_000,
    ).toISOString();
    const resume = within(dialog).getByTestId('vault-hibernation-dialog-resume');
    expect(resume.querySelector('time')).toHaveAttribute('datetime', expected);
  });

  it('cancels without touching the contract', () => {
    const contract = makeContract();
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="active"
        contract={contract}
      />,
    );

    fireEvent.click(screen.getByTestId('vault-hibernation-toggle-button'));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(contract.enterHibernation).not.toHaveBeenCalled();
    expect(contract.exitHibernation).not.toHaveBeenCalled();
  });

  it('calls enter_hibernation with the owner and configured duration', async () => {
    const contract = makeContract();
    const onChanged = jest.fn();
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="active"
        contract={contract}
        durationSeconds={172_800}
        onChanged={onChanged}
      />,
    );

    fireEvent.click(screen.getByTestId('vault-hibernation-toggle-button'));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Hibernate vault' }));

    await waitFor(() => {
      expect(contract.enterHibernation).toHaveBeenCalledWith(VAULT_ID, OWNER, 172_800);
    });
    expect(contract.exitHibernation).not.toHaveBeenCalled();
    expect(onChanged).toHaveBeenCalledWith('hibernating');
    await waitFor(() => {
      expect(screen.getByTestId('vault-hibernation-status')).toHaveTextContent('Hibernating');
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
  });

  it('calls exit_hibernation when ending hibernation', async () => {
    const contract = makeContract();
    const onChanged = jest.fn();
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="hibernating"
        entry={ENTRY}
        contract={contract}
        onChanged={onChanged}
      />,
    );

    fireEvent.click(screen.getByTestId('vault-hibernation-toggle-button'));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'End hibernation' }));

    await waitFor(() => {
      expect(contract.exitHibernation).toHaveBeenCalledWith(VAULT_ID, OWNER);
    });
    expect(contract.enterHibernation).not.toHaveBeenCalled();
    expect(onChanged).toHaveBeenCalledWith('active');
  });

  it('surfaces contract failures and keeps the dialog open', async () => {
    const contract = makeContract({
      enterHibernation: jest.fn().mockRejectedValue(new Error('transaction failed')),
    });
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="active"
        contract={contract}
      />,
    );

    fireEvent.click(screen.getByTestId('vault-hibernation-toggle-button'));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Hibernate vault' }));

    expect(await screen.findByTestId('vault-hibernation-error')).toHaveTextContent(
      'transaction failed',
    );
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByTestId('vault-hibernation-status')).toHaveTextContent('Active');
  });

  it('closes the dialog on Escape', () => {
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="active"
        contract={makeContract()}
      />,
    );

    fireEvent.click(screen.getByTestId('vault-hibernation-toggle-button'));
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('disables the toggle until a wallet is connected', () => {
    render(<VaultHibernationToggle vaultId={VAULT_ID} status="active" contract={makeContract()} />);

    expect(screen.getByTestId('vault-hibernation-toggle-button')).toBeDisabled();
  });

  it('disables the toggle while the hibernation state is unknown', () => {
    render(
      <VaultHibernationToggle
        vaultId={VAULT_ID}
        owner={OWNER}
        status="unknown"
        contract={makeContract()}
      />,
    );

    expect(screen.getByTestId('vault-hibernation-status')).toHaveTextContent('Status unknown');
    expect(screen.getByTestId('vault-hibernation-toggle-button')).toBeDisabled();
  });
});
