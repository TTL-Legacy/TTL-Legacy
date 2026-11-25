/**
 * Hook for persisting the vault creation wizard draft state in localStorage.
 *
 * Issue #2067.
 */

import { useCallback, useEffect, useState } from "react";
import {
  DEFAULT_DRAFT,
  WizardDraft,
  WizardStep,
} from "./types";

const STORAGE_KEY = "ttl_vault_creation_draft";

function loadDraft(): WizardDraft | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as WizardDraft;
    // Minimal validation: must have a savedAt field.
    if (!parsed.savedAt) return null;
    return parsed;
  } catch {
    return null;
  }
}

function saveDraft(draft: WizardDraft): void {
  try {
    const withTimestamp: WizardDraft = {
      ...draft,
      savedAt: new Date().toISOString(),
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(withTimestamp));
  } catch {
    // localStorage may be unavailable in some environments; silently ignore.
  }
}

function clearDraft(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // ignore
  }
}

export interface UseWizardDraftReturn {
  draft: WizardDraft;
  hasSavedDraft: boolean;
  updateDraft: (patch: Partial<WizardDraft>) => void;
  setStep: (step: WizardStep) => void;
  discardDraft: () => void;
}

export function useWizardDraft(): UseWizardDraftReturn {
  const [draft, setDraft] = useState<WizardDraft>(() => {
    const saved = loadDraft();
    return saved ?? { ...DEFAULT_DRAFT, savedAt: new Date().toISOString() };
  });

  const [hasSavedDraft] = useState<boolean>(() => loadDraft() !== null);

  // Persist on every change.
  useEffect(() => {
    saveDraft(draft);
  }, [draft]);

  const updateDraft = useCallback((patch: Partial<WizardDraft>) => {
    setDraft((prev) => ({ ...prev, ...patch }));
  }, []);

  const setStep = useCallback((step: WizardStep) => {
    setDraft((prev) => ({ ...prev, currentStep: step }));
  }, []);

  const discardDraft = useCallback(() => {
    clearDraft();
    setDraft({ ...DEFAULT_DRAFT, savedAt: new Date().toISOString() });
  }, []);

  return { draft, hasSavedDraft, updateDraft, setStep, discardDraft };
}
