import { formatDateTime } from "../core/format-date";
import { StatusMessage } from "../core/status-message";
import { SettingToggle } from "./setting-toggle";
import {
  useTypingStatisticsUpload,
  type UseTypingStatisticsUploadOptions,
} from "./use-typing-statistics-upload";

export { uploadErrorMessage } from "./use-typing-statistics-upload";

/**
 * 打字统计页的「上传到云端」。挂上时调一次 `sync`（宿主十分钟内只真正上传一次），所以打开统计页就会补传；开关打开时立刻上传，关掉时删除云端副本。`onEnabledChange` 把开关状态交给页面，页面据此改写「只保存在本机」的说明。
 */
export function TypingStatisticsUpload({
  client,
  onEnabledChange,
}: UseTypingStatisticsUploadOptions) {
  const { status, busy, message, toggle } = useTypingStatisticsUpload({ client, onEnabledChange });

  const signedIn = status?.signedIn ?? false;
  const enabled = status?.enabled ?? false;
  return (
    <>
      <SettingToggle
        label="上传到云端"
        description="把每天的字数、活跃时长、时段、输入方式和候选位次上传到你的水杉账号，供 QQ 群助手的 /stats 查看。只有计数，不含输入内容；关闭时删除云端的数据。"
        detail={signedIn || enabled ? undefined : "登录水杉账号后可以打开"}
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
