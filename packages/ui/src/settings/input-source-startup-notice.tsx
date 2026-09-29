export type InputSourceStartupStatus = {
  /** `login_required`: the input method is installed, but this login session's input source list only picks it up after the user logs in again. */
  action: "installed" | "updated" | "up_to_date" | "login_required" | "failed";
  /** Whether the input source is in the System Settings list; `null` when that list could not be read. */
  enabled: boolean | null;
  bundled_version: string | null;
  installed_version: string | null;
};

export interface InputSourceStartupNoticeProps {
  status: InputSourceStartupStatus;
  onOpenSettings: () => void | Promise<void>;
  onDismiss: () => void;
  onError: (message: string) => void;
}

/** Shows the macOS input-source installation result reported during startup. */
export function InputSourceStartupNotice({
  status,
  onOpenSettings,
  onDismiss,
  onError,
}: InputSourceStartupNoticeProps) {
  return (
    <div
      role={status.action === "failed" ? "alert" : "status"}
      className={status.action === "failed" ? "error" : "notice"}
      aria-label="水杉输入法安装状态"
    >
      {status.action === "installed" && (
        <p>水杉输入法已安装{status.installed_version ? `：${status.installed_version}` : ""}。</p>
      )}
      {status.action === "updated" && (
        <p>水杉输入法已更新{status.installed_version ? `到 ${status.installed_version}` : ""}。</p>
      )}
      {status.action === "login_required" && (
        <p>
          水杉输入法已安装到本机，但本次登录的输入法列表还看不到它。请注销并重新登录，然后在
          系统设置 &gt; 键盘 &gt; 输入法 中添加水杉输入法。
        </p>
      )}
      {status.action === "failed" && (
        <p>
          水杉输入法未能自动安装或更新。请在「快捷键」页的「输入法服务」中点「安装 / 更新」重试。
        </p>
      )}
      {status.enabled === false && status.action !== "login_required" && (
        <p>
          请在 系统设置 &gt; 键盘 &gt; 输入法 中添加并启用水杉输入法。
          <button
            type="button"
            className="secondary"
            onClick={() => {
              void Promise.resolve(onOpenSettings()).catch(() =>
                onError("无法打开系统设置，请手动前往 系统设置 > 键盘 > 输入法。"),
              );
            }}
          >
            打开键盘设置
          </button>
        </p>
      )}
      <button type="button" className="secondary" onClick={onDismiss}>
        知道了
      </button>
    </div>
  );
}
