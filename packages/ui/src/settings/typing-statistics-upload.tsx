import { useEffect, useState } from "react";
import { errorCode } from "../core/error-code";
import { formatDateTime } from "../core/format-date";
import { StatusMessage } from "../core/status-message";
import { SettingToggle } from "./setting-toggle";
import { useAsyncGeneration } from "./use-async-generation";
import { useMountedRef } from "./use-mounted-ref";
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

/**
 * 打字统计页的「上传到云端」。挂上时调一次 `sync`（宿主十分钟内只真正上传一次），所以打开统计页就会补传；开关打开时立刻上传，关掉时删除云端副本。`onEnabledChange` 把开关状态交给页面，页面据此改写「只保存在本机」的说明。
 */
export function TypingStatisticsUpload({
  client,
  onEnabledChange,
}: {
  client: TypingStatisticsUploadClient;
  onEnabledChange?: (enabled: boolean) => void;
}) {
  const [status, setStatus] = useState<TypingStatisticsUploadStatus>();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const mounted = useMountedRef();
  const generation = useAsyncGeneration(client);

  const apply = (next: TypingStatisticsUploadStatus) => {
    setStatus(next);
    onEnabledChange?.(next.enabled);
  };

  useEffect(() => {
    const current = generation.current;
    setMessage("");
    void client.sync().then(
      (next) => {
        if (mounted.current && generation.current === current) apply(next);
      },
      () =>
        // 自动补传失败不打扰用户，只读出开关状态。
        client.status().then(
          (next) => {
            if (mounted.current && generation.current === current) apply(next);
          },
          () => undefined,
        ),
    );
  }, [client]);

  const toggle = async (enabled: boolean) => {
    if (busy) return;
    const current = generation.current;
    setBusy(true);
    setMessage("");
    try {
      const next = await client.setEnabled(enabled);
      if (!mounted.current || generation.current !== current) return;
      apply(next);
      setMessage(
        enabled
          ? "已打开上传，最近 7 天的每日计数已传到你的水杉账号。"
          : "已关闭上传，云端的打字统计已删除。",
      );
    } catch (error) {
      if (!mounted.current || generation.current !== current) return;
      setMessage(uploadErrorMessage(error, enabled));
      // 开关在宿主那边可能已经变了（打开后上传失败、关闭后删除失败），重新读一次。
      await client.status().then(apply, () => undefined);
    } finally {
      if (mounted.current && generation.current === current) setBusy(false);
    }
  };

  const signedIn = status?.signedIn ?? false;
  const enabled = status?.enabled ?? false;
  return (
    <>
      <SettingToggle
        label="上传到云端"
        description={
          signedIn || enabled
            ? "把每天的字数、活跃时长、时段、输入方式和候选位次上传到你的水杉账号，供 QQ 群助手的 /stats 查看。只有计数，不含输入内容；关闭时删除云端的数据。"
            : "登录水杉账号后可以打开，供 QQ 群助手的 /stats 查看。只有计数，不含输入内容。"
        }
        ariaLabel="上传到云端"
        rowClassName="mb-4"
        compact
        checked={enabled}
        // 已经打开的开关在退出登录后仍能关掉。
        disabled={busy || !status || (!signedIn && !enabled)}
        onChange={(next) => void toggle(next)}
      />
      {enabled && status?.lastUploadMs ? (
        <p className="-mt-2 mb-4 text-xs [color:var(--p-sub)]">
          上次上传：{formatDateTime(status.lastUploadMs)}
        </p>
      ) : null}
      {message && <StatusMessage role="status">{message}</StatusMessage>}
    </>
  );
}
