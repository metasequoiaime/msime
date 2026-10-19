import { useCallback, useEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useLatestRef } from "../core/use-latest-ref";
import type {
  TypingStatisticsUploadClient,
  TypingStatisticsUploadStatus,
} from "./typing-statistics-model";

/** 上传和删除失败时页面上的说法。 */
export function uploadErrorMessage(error: unknown, enabling: boolean): string {
  switch (errorCode(error)) {
    case "account_unauthorized":
      return "先在「账号」页登录水杉账号，再打开上传。";
    case "account_rate_limited":
      return "上传过于频繁，过一会儿会自动再传。";
    case "storage":
      return "无法读取或保存上传设置，请稍后重试。";
  }
  return enabling
    ? "已打开上传，但这次没有传上去，稍后会自动再试。"
    : "已关闭上传，但云端的数据没有删掉，请联网后打开再关闭一次。";
}

export interface UseTypingStatisticsUploadOptions {
  client: TypingStatisticsUploadClient;
  onEnabledChange?: (enabled: boolean) => void;
}

export interface TypingStatisticsUploadState {
  status: TypingStatisticsUploadStatus | undefined;
  busy: boolean;
  message: string;
  toggle: (enabled: boolean) => Promise<void> | undefined;
}

/** 管理挂载时补传、开关变更和失败后的状态补读；只公布当前客户端的结果。 */
export function useTypingStatisticsUpload({
  client,
  onEnabledChange,
}: UseTypingStatisticsUploadOptions): TypingStatisticsUploadState {
  const [status, setStatus] = useState<TypingStatisticsUploadStatus>();
  const [message, setMessage] = useState("");
  const notify = useLatestRef(onEnabledChange);
  const { busy, run, mounted, generation } = useAsyncActionRunner(setMessage, undefined, client);
  const readVersion = useRef(0);
  const apply = useCallback(
    (next: TypingStatisticsUploadStatus) => {
      setStatus(next);
      notify.current?.(next.enabled);
    },
    [notify],
  );

  useEffect(() => {
    const current = generation.current;
    const version = ++readVersion.current;
    const isCurrent = () =>
      mounted.current && generation.current === current && readVersion.current === version;
    setStatus(undefined);
    setMessage("");
    const sync = async () => {
      try {
        const next = await client.sync();
        if (isCurrent()) apply(next);
      } catch {
        if (!isCurrent()) return;
        // 自动补传失败不打扰用户，只读出开关状态；补读也必须检查客户端和请求归属。
        try {
          const next = await client.status();
          if (isCurrent()) apply(next);
        } catch {
          // 没有读到状态时保持未知，由视图禁用开关。
        }
      }
    };
    void sync();
  }, [client, generation, mounted, apply]);

  const toggle = (enabled: boolean) =>
    run(
      async (isCurrent) => {
        // 开关变更优先于之前启动的自动补传，避免旧快照覆盖变更返回值。
        readVersion.current++;
        try {
          const next = await client.setEnabled(enabled);
          if (!isCurrent()) return;
          apply(next);
          setMessage(
            enabled
              ? "已打开上传，最近 7 天的每日计数已传到你的水杉账号。"
              : "已关闭上传，云端的打字统计已删除。",
          );
        } catch (error) {
          if (!isCurrent()) return;
          setMessage(uploadErrorMessage(error, enabled));
          // 宿主可能已改开关但上传或删除失败，补读实际状态，迟到结果仍需检查代次。
          try {
            const next = await client.status();
            if (isCurrent()) apply(next);
          } catch {
            // 补读失败不覆盖本次上传或删除的错误说明。
          }
        }
      },
      { formatError: () => "" },
    );

  return { status, busy, message, toggle };
}
