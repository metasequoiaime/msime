import { useEffect, useRef, useState } from "react";
import type { DictionaryClient, DictionaryUpdateStatus } from "../dictionary/dictionary-contracts";
import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";
import { SummaryRow } from "./summary-row";
import { ActionRow } from "./action-row";
import { SettingsGroupNote } from "./settings-group-note";

/** 开关只控制之后的自动检查，已下载的词库保留；手动检查由宿主启动后台任务。 */
export function DictionaryUpdates({
  request,
}: {
  request: NonNullable<DictionaryClient["updates"]>;
}) {
  const [status, setStatus] = useState<DictionaryUpdateStatus | null>(null);
  const [error, setError] = useState("");
  const generation = useRef(0);
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
    const timer = setInterval(() => {
      void action("status");
    }, 2000);
    return () => {
      generation.current++;
      clearInterval(timer);
    };
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
  return (
    <GroupList title="词库更新">
      <SwitchRow
        title="自动更新词库"
        checked={status?.enabled ?? true}
        disabled={!status}
        onChange={(enabled) => {
          void action("configure", enabled);
        }}
      />
      <SummaryRow title="词库版本">{status?.version ?? "读取中…"}</SummaryRow>
      <ActionRow
        title="检查词库更新"
        label={running ? "更新中…" : "检查更新"}
        disabled={running || !status}
        action={() => action("check")}
      />
      {status?.can_rollback && (
        <ActionRow
          title="恢复上一版词库"
          description="恢复后关闭自动更新，下次启动生效"
          label="恢复"
          disabled={running}
          action={() => action("rollback")}
        />
      )}
      <SettingsGroupNote>{message}</SettingsGroupNote>
    </GroupList>
  );
}
