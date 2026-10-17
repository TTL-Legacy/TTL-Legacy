import React, { useState } from 'react';

export interface TokenAllowlistAdminProps {
  /** Token contract addresses currently in the on-chain allowlist (`get_allowed_tokens`) */
  tokens: string[];
  /** Only the contract admin sees the panel; non-admins get nothing rendered */
  isAdmin: boolean;
  /** Called with a validated token address; should reject if the contract call fails */
  onAdd: (token: string) => Promise<void>;
  /** Called with a token address currently in the allowlist */
  onRemove: (token: string) => Promise<void>;
  className?: string;
}

/** Soroban contract addresses are 56-char StrKey strings starting with `C`. */
const CONTRACT_ADDRESS_PATTERN = /^C[A-Z2-7]{55}$/;

/**
 * Admin panel for the token allowlist (TOKEN_ALLOWLIST.md, Issue #1118).
 *
 * Lists allowed tokens with per-row remove controls and an add form. The
 * contract enforces admin auth; `isAdmin` only controls what is shown.
 */
export function TokenAllowlistAdmin({ tokens, isAdmin, onAdd, onRemove, className }: TokenAllowlistAdminProps) {
  const [input, setInput] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  if (!isAdmin) {
    return null;
  }

  const runAction = async (action: () => Promise<void>, failure: string): Promise<boolean> => {
    setPending(true);
    setError(null);
    try {
      await action();
      return true;
    } catch {
      setError(failure);
      return false;
    } finally {
      setPending(false);
    }
  };

  const handleAdd = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const token = input.trim();

    if (!CONTRACT_ADDRESS_PATTERN.test(token)) {
      setError('Enter a valid token contract address (starts with C, 56 characters).');
      return;
    }

    if (tokens.includes(token)) {
      setError('This token is already allowed.');
      return;
    }

    if (await runAction(() => onAdd(token), 'Failed to add token to the allowlist.')) {
      setInput('');
    }
  };

  return (
    <section className={className} aria-labelledby="token-allowlist-admin-heading" data-testid="token-allowlist-admin">
      <h2 id="token-allowlist-admin-heading">Token allowlist</h2>

      {tokens.length === 0 ? (
        <p>No tokens are allowed yet. Deposits will be rejected until one is added.</p>
      ) : (
        <ul aria-label="Allowed tokens">
          {tokens.map((token) => (
            <li key={token}>
              <code>{token}</code>{' '}
              <button
                type="button"
                onClick={() => runAction(() => onRemove(token), 'Failed to remove token from the allowlist.')}
                disabled={pending}
                aria-label={`Remove ${token} from allowlist`}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      <form onSubmit={handleAdd} noValidate>
        <label htmlFor="token-allowlist-input">Token contract address</label>{' '}
        <input
          id="token-allowlist-input"
          type="text"
          value={input}
          onChange={(event) => setInput(event.target.value)}
          disabled={pending}
          autoComplete="off"
          spellCheck={false}
        />{' '}
        <button type="submit" disabled={pending || input.trim().length === 0}>
          Add token
        </button>
      </form>

      {error && (
        <p role="alert" style={{ color: '#d32f2f' }}>
          {error}
        </p>
      )}
    </section>
  );
}

export default TokenAllowlistAdmin;
