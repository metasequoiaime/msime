import { useEffect, useState } from "react";
import type { DictionaryClient, DictionaryUpdateStatus } from "../dictionary/dictionary-contracts";
import { subscribeToInterval } from "../core/use-interval";
import { useAsyncGeneration } from "./use-async-generation";

/** 管理词库更新状态、两秒轮询与最新请求结果；实际检查、配置和回退由共享宿主执行。 */
export function useDictionaryUpdates(request: NonNullable<DictionaryClient["updates"]>) {
  const [status, setStatus] = useState<DictionaryUpdateStatus | null>(null);
  const [error, setError] = useState("");
  const generation = useAsyncGeneration(request);
  const action = async (command: Parameters<typeof request>[0], enabled?: boolean) => {
    const current = ++generation.current;
    try {
      const next = await request(command, enabled);
      if (current === generation.current) {
        setStatus(next);
        setError("");
      }
    } catch {
      if (current === generation.current) setError("词库更新暂不可用，请稍后重试");
    }
  };
  useEffect(() => {
    void action("status");
    const stopInterval = subscribeToInterval(() => {
      void action("status");
    }, 2000);
    return stopInterval;
  }, [request]);
  const running = status?.phase === "checking" || status?.phase === "downloading";
  const message =
    error ||
    (status?.error
      ? "更新失败，继续使用原词库"
      : status?.phase === "checking"
        ? "正在检查词库更新…"
        : status?.phase === "downloading"
          ? `正在下载 ${Math.floor(status.downloaded_bytes / 1048576)} / ${Math.ceil(status.total_bytes / 1048576)} MiB`
          : status?.pending_restart
            ? "词库已准备好，下次启动输入法或重新打开键盘时生效"
            : status?.phase === "current"
              ? "词库已是最新版本"
              : "每天自动检查兼容的词库版本，个人词条和学习记录保留");
  return { status, running, message, action };
}
