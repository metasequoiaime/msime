import * as settings from "./settings-style";

export interface DataDirectoryInfo {
  path: string;
  isDefault: boolean;
}

export interface DataDirectorySectionProps {
  visible: boolean;
  linux: boolean;
  dataDirectory?: DataDirectoryInfo;
  busy: boolean;
  result: string;
  onChoose: () => void;
}

/** Data directory status and relocation action shared by macOS and Linux settings. */
export function DataDirectorySection({
  visible,
  linux,
  dataDirectory,
  busy,
  result,
  onChoose,
}: DataDirectorySectionProps) {
  if (!visible) return null;

  return (
    <div className="section" role="group" aria-label="数据目录">
      <div className="section-header">
        <span className="section-title">
          数据目录
          <small>
            词库、学习记录、皮肤、剪贴板历史和设置共用此位置。可移动到其他磁盘。
            {linux &&
              "输入法入口配置和在线服务、语音服务的凭据固定保存在 ~/.config/msime-client，不随数据移动。"}
          </small>
        </span>
      </div>
      <div className={settings.serviceRow}>
        <span>
          当前目录
          <small>
            <code>{dataDirectory?.path ?? "正在读取…"}</code>
            {dataDirectory?.isDefault ? "（默认）" : ""}
          </small>
        </span>
        <div>
          <button
            type="button"
            className="secondary"
            disabled={busy || !dataDirectory}
            aria-busy={busy}
            onClick={onChoose}
          >
            {busy ? "正在移动…" : "选择位置…"}
          </button>
        </div>
      </div>
      {result && <p role="status">{result}</p>}
    </div>
  );
}
