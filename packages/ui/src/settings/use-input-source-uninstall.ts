import { useState } from "react";

export interface UseInputSourceUninstallOptions {
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
}

/** Owns the confirmation and result state for removing the native input source. */
export function useInputSourceUninstall({ uninstallInputSource }: UseInputSourceUninstallOptions) {
  const [removeUserData, setRemoveUserData] = useState(false);
  const [confirmation, setConfirmation] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<"success" | "error" | null>(null);

  async function confirmUninstall() {
    if (!uninstallInputSource || busy) return;
    setBusy(true);
    setResult(null);
    try {
      await uninstallInputSource(removeUserData);
      setConfirmation(false);
      setResult("success");
    } catch {
      setResult("error");
    } finally {
      setBusy(false);
    }
  }

  function requestUninstall() {
    setResult(null);
    setConfirmation(true);
  }

  return {
    busy,
    cancelUninstall: () => setConfirmation(false),
    confirmUninstall,
    confirmation,
    removeUserData,
    requestUninstall,
    result,
    setRemoveUserData,
  } as const;
}
