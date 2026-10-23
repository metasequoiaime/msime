import { useState } from "react";
import { errorCode } from "../core/error-code";
import { useAsyncActionRunner } from "../core/use-async-action";

/** 宿主失败可能包含私人路径或数据，界面只显示约定提示。 */
export function HostActionButton({
  action,
  label,
  success,
  error,
  cancelled,
}: {
  action?: () => Promise<void>;
  label: string;
  success?: string;
  error?: string;
  cancelled?: string;
}) {
  const [result, setResult] = useState<"success" | "error" | null>(null);
  const [errorMessage, setErrorMessage] = useState("操作失败，请重试。");
  const runner = useAsyncActionRunner(setErrorMessage, undefined);
  const pending = runner.busy;
  function run() {
    if (!action) return;
    return runner.run(
      async (isCurrent) => {
        setResult(null);
        await action();
        if (isCurrent()) setResult("success");
      },
      {
        formatError: (reason) => {
          const code = errorCode(reason);
          if (code === "cancelled") return cancelled ?? "已取消操作，可以重试。";
          return code === "unknown_skin"
            ? "找不到该外部皮肤，请先刷新皮肤目录。"
            : code === "invalid_skin"
              ? "皮肤标识无效，未执行应用。"
              : (error ?? "操作失败，请重试。");
        },
        onError: () => setResult("error"),
      },
    );
  }
  return (
    <div>
      <button
        type="button"
        className="secondary"
        disabled={!action || pending}
        aria-label={label}
        aria-busy={pending}
        onClick={() => void run()}
      >
        {pending ? "处理中…" : label}
      </button>
      {result === "success" && success && <span role="status">{success}</span>}
      {result === "error" && <span role="alert">{errorMessage}</span>}
    </div>
  );
}
