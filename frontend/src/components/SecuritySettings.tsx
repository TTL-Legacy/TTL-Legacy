import { useCallback, useEffect, useState } from 'react';
import type { Passkey } from '../api/generated';
import { apiClient } from '../api/client';

export interface SecuritySettingsProps {
  /** Passkeys to display. When omitted, the component loads them itself. */
  passkeys?: Passkey[];
  /** Vault id used to load passkeys when `passkeys` is omitted. */
  vaultId?: string;
  /** Owner (Stellar address) used to load passkeys when `passkeys` is omitted. */
  owner?: string;
}

/**
 * Security settings page (Issue #2147): shows registered passkeys with their
 * user-editable `friendly_name`, falling back to the AAGUID-derived
 * `device_name` when no custom name is set. Users can rename a passkey via
 * the edit flow, which calls `PUT /auth/passkeys/{id}/name`.
 */
export function SecuritySettings({
  passkeys: initialPasskeys,
  vaultId,
  owner,
}: SecuritySettingsProps) {
  const [passkeys, setPasskeys] = useState<Passkey[]>(initialPasskeys ?? []);
  const [loading, setLoading] = useState(!initialPasskeys);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draftName, setDraftName] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const displayName = (pk: Passkey) => pk.friendly_name?.trim() || pk.device_name;

  const refreshPasskeys = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list =
        vaultId && owner ? await apiClient.listPasskeys(vaultId, owner) : [];
      setPasskeys(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load passkeys');
    } finally {
      setLoading(false);
    }
  }, [vaultId, owner]);

  useEffect(() => {
    if (!initialPasskeys) {
      void refreshPasskeys();
    }
  }, [initialPasskeys, refreshPasskeys]);

  const startEdit = (pk: Passkey) => {
    setEditingId(pk.passkey_id);
    setDraftName(displayName(pk));
    setError(null);
  };

  const cancelEdit = () => {
    setEditingId(null);
    setDraftName('');
    setError(null);
  };

  const saveName = async (pk: Passkey) => {
    const friendlyName = draftName.trim();
    setSaving(true);
    setError(null);
    try {
      const updated = await apiClient.updatePasskeyFriendlyName(pk.passkey_id, {
        friendly_name: friendlyName,
      });
      setPasskeys((prev) =>
        prev.map((p) => (p.passkey_id === updated.passkey_id ? updated : p)),
      );
      setEditingId(null);
      setDraftName('');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save passkey name');
    } finally {
      setSaving(false);
    }
  };

  return (
    <section data-testid="security-settings" className="security-settings">
      <h2>Security settings</h2>

      {error && (
        <p role="alert" data-testid="security-passkey-error" className="security-error">
          {error}
        </p>
      )}

      <h3>Passkeys</h3>
      {loading && <p data-testid="security-passkeys-loading">Loading passkeys…</p>}
      <ul data-testid="security-passkeys" className="security-passkeys">
        {passkeys.map((pk) => (
          <li key={pk.passkey_id} data-testid={`passkey-${pk.passkey_id}`}>
            {editingId === pk.passkey_id ? (
              <form
                data-testid={`edit-form-${pk.passkey_id}`}
                onSubmit={(e) => {
                  e.preventDefault();
                  void saveName(pk);
                }}
              >
                <label htmlFor={`friendly-name-${pk.passkey_id}`}>Passkey name</label>
                <input
                  id={`friendly-name-${pk.passkey_id}`}
                  data-testid={`friendly-name-input-${pk.passkey_id}`}
                  value={draftName}
                  onChange={(e) => setDraftName(e.target.value)}
                />
                <button type="submit" disabled={saving} data-testid={`save-name-${pk.passkey_id}`}>
                  {saving ? 'Saving…' : 'Save'}
                </button>
                <button
                  type="button"
                  data-testid={`cancel-edit-${pk.passkey_id}`}
                  onClick={cancelEdit}
                >
                  Cancel
                </button>
              </form>
            ) : (
              <>
                <span data-testid={`passkey-name-${pk.passkey_id}`} className="passkey-name">
                  {displayName(pk)}
                </span>
                {pk.friendly_name && (
                  <span data-testid={`passkey-device-${pk.passkey_id}`} className="passkey-device">
                    {pk.device_name}
                  </span>
                )}
                <button
                  type="button"
                  data-testid={`edit-name-${pk.passkey_id}`}
                  onClick={() => startEdit(pk)}
                >
                  Edit
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

export default SecuritySettings;
