import { useState } from "react";
import { errorCode } from "../core/error-code";
import { useAsyncActionRunner } from "../core/use-async-action";

export interface UseInputSourceUninstallOptions {
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
}

/** 管理原生输入源卸载的确认和结果。 */
export function useInputSourceUninstall({ uninstallInputSource }: UseInputSourceUninstallOptions) {
  const [removeUserData, setRemoveUserData] = useState(false);
  const [confirmation, setConfirmation] = useState(false);
  const [result, setResult] = useState<"success" | "error" | "listed" | "system" | null>(null);
  const { busy, run } = useAsyncActionRunner(
    (message) =>
      setResult(message === "listed" || message === "system" ? message : message ? "error" : null),
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
      // 系统副本需手动移除；仍在输入源列表中的用户副本需先从系统设置移除。
      {
        formatError: (error) => {
          const code = errorCode(error);
          return code === "system_input_source"
            ? "system"
            : code === "input_source_listed"
              ? "listed"
              : "error";
        },
      },
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
