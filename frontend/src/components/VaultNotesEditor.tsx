/**
 * VaultNotesEditor — inline markdown editor with live preview (Issue #2055).
 *
 * Displays vault notes as a live-rendered markdown preview by default.
 * Clicking "Edit" switches to a textarea; clicking "Save" calls `onSave`
 * (which should forward to the contract's `set_vault_notes`) and returns to
 * preview mode.  "Cancel" discards unsaved changes.
 *
 * The markdown renderer is intentionally lightweight: it handles the
 * formatting patterns described in #2055 (headers, bullet lists, code blocks,
 * bold, and italics) without pulling in an external library.  HTML output is
 * inserted via `dangerouslySetInnerHTML`; the content is first sanitized
 * through the `sanitizeMarkdown` helper that strips raw HTML tags before
 * parsing so vault-owner-supplied notes cannot inject arbitrary markup.
 *
 * Usage:
 *
 * ```tsx
 * <VaultNotesEditor
 *   vaultId="12345"
 *   initialNotes={vault.notes}
 *   maxLength={1024}
 *   onSave={async (notes) => { await contract.setVaultNotes(vaultId, notes); }}
 * />
 * ```
 */
import React, { useCallback, useId, useState } from 'react';

// ── Markdown renderer ────────────────────────────────────────────────────────

/**
 * Remove raw HTML tags so notes cannot inject markup into the preview pane.
 * Actual markdown delimiters are preserved.
 */
function sanitizeMarkdown(raw: string): string {
  // Strip anything that looks like an HTML tag.
  return raw.replace(/<\/?[a-zA-Z][^>]*>/g, '');
}

/**
 * Convert a subset of Markdown to an HTML string suitable for
 * `dangerouslySetInnerHTML`.  Supported constructs:
 *
 * - Fenced code blocks (``` … ```)
 * - ATX headings (# / ## / ###)
 * - Unordered lists (lines starting with `- ` or `* `)
 * - Bold (**text** or __text__)
 * - Italic (*text* or _text_)
 * - Inline code (`code`)
 */
export function renderMarkdown(markdown: string): string {
  const safe = sanitizeMarkdown(markdown);
  const lines = safe.split('\n');
  const out: string[] = [];

  let inCodeBlock = false;
  let codeLines: string[] = [];
  let inList = false;

  const flushList = () => {
    if (inList) {
      out.push('</ul>');
      inList = false;
    }
  };

  const inlineFormat = (text: string): string =>
    text
      // Bold: **text** or __text__
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/__(.+?)__/g, '<strong>$1</strong>')
      // Italic: *text* or _text_ (not preceded/followed by another * or _)
      .replace(/(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g, '<em>$1</em>')
      .replace(/(?<!_)_(?!_)(.+?)(?<!_)_(?!_)/g, '<em>$1</em>')
      // Inline code
      .replace(/`([^`]+)`/g, '<code>$1</code>');

  for (const line of lines) {
    // ── Fenced code block ──
    if (line.trimStart().startsWith('```')) {
      if (inCodeBlock) {
        // Close code block
        out.push(`<pre><code>${codeLines.map(escapeHtml).join('\n')}</code></pre>`);
        codeLines = [];
        inCodeBlock = false;
      } else {
        flushList();
        inCodeBlock = true;
      }
      continue;
    }

    if (inCodeBlock) {
      codeLines.push(line);
      continue;
    }

    // ── ATX Heading ──
    const headingMatch = line.match(/^(#{1,3})\s+(.+)/);
    if (headingMatch) {
      flushList();
      const level = headingMatch[1].length;
      out.push(`<h${level}>${inlineFormat(escapeHtml(headingMatch[2]))}</h${level}>`);
      continue;
    }

    // ── Unordered list item ──
    const listMatch = line.match(/^[-*]\s+(.+)/);
    if (listMatch) {
      if (!inList) {
        out.push('<ul>');
        inList = true;
      }
      out.push(`<li>${inlineFormat(escapeHtml(listMatch[1]))}</li>`);
      continue;
    }

    // ── Blank line ──
    if (line.trim() === '') {
      flushList();
      out.push('<br />');
      continue;
    }

    // ── Plain paragraph line ──
    flushList();
    out.push(`<p>${inlineFormat(escapeHtml(line))}</p>`);
  }

  // Close any open code block or list at EOF
  if (inCodeBlock && codeLines.length > 0) {
    out.push(`<pre><code>${codeLines.map(escapeHtml).join('\n')}</code></pre>`);
  }
  flushList();

  return out.join('');
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// ── Component ────────────────────────────────────────────────────────────────

export interface VaultNotesEditorProps {
  vaultId: string;
  /** Current notes text as stored on-chain.  May be empty. */
  initialNotes?: string;
  /** Maximum allowed length in characters (mirrors contract MAX_NOTES_LEN = 1024). */
  maxLength?: number;
  /**
   * Called with the new notes string when the owner clicks Save.
   * Should invoke the contract's `set_vault_notes` and reject on failure.
   */
  onSave?: (notes: string) => Promise<void>;
  /** Whether the current user is the vault owner (hides Edit when false). */
  isOwner?: boolean;
  className?: string;
}

/**
 * Inline vault notes editor with live markdown preview.
 *
 * Shows a formatted preview by default.  Owners switch to edit mode via the
 * "Edit" button; saving calls `onSave`.  Unsaved changes are discarded on
 * "Cancel".
 */
export function VaultNotesEditor({
  vaultId,
  initialNotes = '',
  maxLength = 1024,
  onSave,
  isOwner = false,
  className,
}: VaultNotesEditorProps) {
  const [notes, setNotes] = useState(initialNotes);
  const [draft, setDraft] = useState(initialNotes);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const textareaId = useId();
  const previewId = useId();

  const handleEdit = useCallback(() => {
    setDraft(notes);
    setError(null);
    setIsEditing(true);
  }, [notes]);

  const handleCancel = useCallback(() => {
    setDraft(notes);
    setError(null);
    setIsEditing(false);
  }, [notes]);

  const handleSave = useCallback(async () => {
    if (!onSave) return;
    setError(null);
    setIsSaving(true);
    try {
      await onSave(draft);
      setNotes(draft);
      setIsEditing(false);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Save failed. Please try again.');
    } finally {
      setIsSaving(false);
    }
  }, [draft, onSave]);

  const handleDraftChange = useCallback(
    (event: React.ChangeEvent<HTMLTextAreaElement>) => {
      const value = event.target.value;
      if (value.length <= maxLength) {
        setDraft(value);
      }
    },
    [maxLength],
  );

  const previewHtml = renderMarkdown(notes);
  const draftHtml = renderMarkdown(draft);

  return (
    <section
      className={className}
      data-testid="vault-notes-editor"
      aria-label={`Notes for vault ${vaultId}`}
      style={{ display: 'flex', flexDirection: 'column', gap: 12 }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <h3 style={{ margin: 0, fontSize: 16 }}>Vault notes</h3>
        {isOwner && !isEditing && (
          <button
            type="button"
            onClick={handleEdit}
            data-testid="vault-notes-edit-button"
            aria-label="Edit vault notes"
          >
            Edit
          </button>
        )}
      </div>

      {isEditing ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          {/* Side-by-side editor and preview */}
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr',
              gap: 12,
              alignItems: 'start',
            }}
          >
            {/* Editor pane */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              <label htmlFor={textareaId} style={{ fontWeight: 600, fontSize: 13 }}>
                Markdown
              </label>
              <textarea
                id={textareaId}
                value={draft}
                onChange={handleDraftChange}
                disabled={isSaving}
                maxLength={maxLength}
                rows={12}
                aria-label="Vault notes markdown source"
                data-testid="vault-notes-textarea"
                style={{
                  resize: 'vertical',
                  fontFamily: 'monospace',
                  fontSize: 13,
                  padding: 8,
                  border: '1px solid #ccc',
                  borderRadius: 4,
                  width: '100%',
                  boxSizing: 'border-box',
                }}
              />
              <span
                aria-live="polite"
                data-testid="vault-notes-char-count"
                style={{
                  fontSize: 12,
                  color: draft.length >= maxLength ? '#d32f2f' : '#757575',
                  alignSelf: 'flex-end',
                }}
              >
                {draft.length} / {maxLength}
              </span>
            </div>

            {/* Live preview pane */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              <span style={{ fontWeight: 600, fontSize: 13 }}>Preview</span>
              <div
                id={previewId}
                role="region"
                aria-label="Markdown preview"
                aria-live="polite"
                data-testid="vault-notes-live-preview"
                // The HTML was produced by our sanitizer + renderer — no user
                // HTML passthrough occurs.
                // eslint-disable-next-line react/no-danger
                dangerouslySetInnerHTML={{ __html: draftHtml }}
                style={{
                  minHeight: 160,
                  border: '1px solid #e0e0e0',
                  borderRadius: 4,
                  padding: 8,
                  background: '#fafafa',
                  overflowY: 'auto',
                }}
              />
            </div>
          </div>

          {error && (
            <p
              role="alert"
              data-testid="vault-notes-error"
              style={{ margin: 0, color: '#d32f2f' }}
            >
              {error}
            </p>
          )}

          <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
            <button
              type="button"
              onClick={handleCancel}
              disabled={isSaving}
              data-testid="vault-notes-cancel-button"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={() => void handleSave()}
              disabled={isSaving || !onSave}
              data-testid="vault-notes-save-button"
            >
              {isSaving ? 'Saving…' : 'Save'}
            </button>
          </div>
        </div>
      ) : (
        <div
          role="region"
          aria-label="Vault notes"
          data-testid="vault-notes-preview"
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={{
            __html: previewHtml || '<p style="color:#9e9e9e;font-style:italic">No notes yet.</p>',
          }}
          style={{
            border: '1px solid #e0e0e0',
            borderRadius: 4,
            padding: 12,
            background: '#fafafa',
            minHeight: 48,
          }}
        />
      )}
    </section>
  );
}

export default VaultNotesEditor;
