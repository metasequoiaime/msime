import { actionRow, primary } from "../account/account-style";

export type InputSourceStartupStatus = {
  /** `login_required`: the input method is installed, but this login session's input source list only picks it up after the user logs in again. `not_installed`: a first install, left for the user to start from the install window. */
  action: "installed" | "updated" | "up_to_date" | "not_installed" | "login_required" | "failed";
  /** Whether the input source is in the System Settings list; `null` when that list could not be read. */
  enabled: boolean | null;
  bundled_version: string | null;
  installed_version: string | null;
  /** Copies of the input method in `/Library/Input Methods`, which compete with the user's copy and need an administrator to remove; read afresh on every request. */
  system_bundles?: string[];
};

export interface InputSourceStartupNoticeProps {
  status: InputSourceStartupStatus;
  onOpenSettings: () => void | Promise<void>;
  onDismiss: () => void;
  onError: (message: string) => void;
}

const settingsPath = "「系统设置 › 键盘 › 文字输入 › 输入法」";
const openSettingsError = `无法打开系统设置，请手动前往${settingsPath}。`;

/** Whether the status leaves the user something to add in System Settings, which is what the settings page keeps re-reading the list for. */
export function inputSourceNeedsAdding(status: InputSourceStartupStatus | null): boolean {
  return (
    status?.enabled === false &&
    status.action !== "failed" &&
    status.action !== "login_required" &&
    status.action !== "not_installed"
  );
}

/** The steps for adding the source. macOS 27 does not let a process enable a keyboard input mode (`TISEnableInputSource` answers noErr and changes nothing, see platforms/macos/README.md), so this is the one step the settings app cannot do for the user. */
function AddSteps() {
  return (
    <ol className="m-0 flex list-decimal flex-col gap-1 pl-5 text-[var(--text-secondary)]">
      <li>点「打开键盘设置」，在「文字输入」下的「输入法」一行点「编辑…」。</li>
      <li>点列表左下角的「+」，在左侧选「简体中文」，再选「水杉输入法」，然后点「添加」。</li>
      <li>在菜单栏的输入法菜单里选「水杉输入法」，或按 Control+空格（或地球键）切换过去。</li>
    </ol>
  );
}

/** Shows the macOS input-source installation result reported during startup, and what is left for the user to do. */
export function InputSourceStartupNotice({
  status,
  onOpenSettings,
  onDismiss,
  onError,
}: InputSourceStartupNoticeProps) {
  const failed = status.action === "failed";
  const loginRequired = status.action === "login_required";
  const needsAdding = inputSourceNeedsAdding(status);
  const version = status.installed_version;

  const openSettings = () => {
    void Promise.resolve(onOpenSettings()).catch(() => onError(openSettingsError));
  };

  let title: string;
  let detail: string | null = null;
  if (failed) {
    title = "水杉输入法没能自动安装或更新";
    detail = "请在「快捷键」页的「输入法服务」中点「安装 / 更新」重试。";
  } else if (status.action === "not_installed") {
    title = "水杉输入法还没有安装到本机";
    detail = "请在「快捷键」页的「输入法服务」中点「安装 / 更新」。";
  } else if (loginRequired) {
    title = "重新登录后才能添加水杉输入法";
    detail = `水杉输入法已装到本机，但 macOS 只在登录时读取新装的输入法，这次登录的输入法列表里还找不到它。请注销并重新登录，然后在${settingsPath}中点「编辑…」添加水杉输入法。以后更新不需要再重新登录。`;
  } else if (needsAdding) {
    const installed =
      status.action === "installed"
        ? `已安装${version ? ` ${version}` : ""}。`
        : status.action === "updated"
          ? `已更新${version ? `到 ${version}` : ""}。`
          : "";
    title = "把水杉输入法加入输入法列表";
    detail = `${installed}macOS 只允许你自己把输入法加入列表，加入后才能切换到水杉输入法：`;
  } else if (status.action === "installed") {
    title = version ? `水杉输入法 ${version} 已安装` : "水杉输入法已安装";
  } else if (status.action === "updated") {
    title = version ? `水杉输入法已更新到 ${version}` : "水杉输入法已更新";
  } else {
    title = "系统目录里多了一份水杉输入法";
  }
  const systemBundles = status.system_bundles?.length
    ? `「/Library/Input Methods」里还有一份水杉输入法（${status.system_bundles.join("、")}），它会让输入法列表出现重复项，或用上较旧的版本。请在「访达」中按 Shift+Command+G 前往该文件夹，把它移到废纸篓（需要管理员密码），然后注销并重新登录。`
    : null;

  return (
    <div
      role={failed ? "alert" : "status"}
      className="section flex flex-col gap-2"
      aria-label="水杉输入法安装状态"
    >
      <p className={`section-title m-0${failed ? " text-[var(--danger-text)]" : ""}`}>{title}</p>
      {detail && <p className="notice m-0">{detail}</p>}
      {needsAdding && (
        <>
          <AddSteps />
          <p className="notice m-0 text-xs">
            添加时 macOS
            会提示开发者可以访问你通过此输入法键入的内容。这是系统对所有第三方输入法都会显示的标准提示，不是水杉额外申请的权限。
          </p>
        </>
      )}
      {systemBundles && <p className="notice m-0">{systemBundles}</p>}
      <div className={`${actionRow} mt-1`}>
        {needsAdding && (
          <button type="button" className={primary} onClick={openSettings}>
            打开键盘设置
          </button>
        )}
        <button type="button" className="secondary" onClick={onDismiss}>
          知道了
        </button>
      </div>
    </div>
  );
}
