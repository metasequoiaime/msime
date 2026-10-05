import { useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";

export interface UseInputSourceUninstallOptions {
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
}

/** Owns the confirmation and result state for removing the native input source. */
export function useInputSourceUninstall({ uninstallInputSource }: UseInputSourceUninstallOptions) {
  const [removeUserData, setRemoveUserData] = useState(false);
  const [confirmation, setConfirmation] = useState(false);
  const [result, setResult] = useState<"success" | "error" | null>(null);
  const { busy, run } = useAsyncActionRunner(
    (message) => setResult(message ? "error" : null),
    undefined,
    uninstallInputSource,
  );

  async function confirmUninstall() {
    if (!uninstallInputSource || busy) return;
    setResult(null);
    await run(
      async (isCurrent) => {
        await uninstallInputSource(removeUserData);
        if (!isCurrent()) return;
        setConfirmation(false);
        setResult("success");
      },
      { formatError: () => "error" },
    );
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
