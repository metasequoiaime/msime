import { useState } from "react";
import { errorCode } from "../core/error-code";
import { useAsyncActionRunner } from "../core/use-async-action";

export interface UseInputSourceUninstallOptions {
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
  cancelInputSourceUninstall?: () => Promise<void>;
}

/** Owns the confirmation and result state for removing the native input source. */
export function useInputSourceUninstall({
  uninstallInputSource,
  cancelInputSourceUninstall,
}: UseInputSourceUninstallOptions) {
  const [removeUserData, setRemoveUserData] = useState(false);
  const [confirmation, setConfirmation] = useState(false);
  const [result, setResult] = useState<"success" | "error" | "listed" | null>(null);
  const { busy, run } = useAsyncActionRunner(
    (message) => setResult(message === "listed" ? "listed" : message ? "error" : null),
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
      // The host refuses while System Settings still lists this input method and opens that page; the user removes it there and confirms again.
      { formatError: (error) => (errorCode(error) === "input_source_listed" ? "listed" : "error") },
    );
  }

  function cancelUninstall() {
    setConfirmation(false);
    // An uninstall that stopped for the user to remove the input sources has kept the input method out of service; give it back.
    if (result === "listed") {
      setResult(null);
      void cancelInputSourceUninstall?.().catch(() => undefined);
    }
  }

  function requestUninstall() {
    setResult(null);
    setConfirmation(true);
  }

  return {
    busy,
    cancelUninstall,
    confirmUninstall,
    confirmation,
    removeUserData,
    requestUninstall,
    result,
    setRemoveUserData,
  } as const;
}
