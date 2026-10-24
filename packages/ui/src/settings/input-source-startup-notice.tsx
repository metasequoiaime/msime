import { actionRow, primary } from "../account/account-style";
import { MascotIllustration } from "../core/mascot";
import { ActionButton } from "./action-button";
import { SettingSectionTitle } from "./setting-section-title";

import {
  inputSourceNeedsAdding,
  type InputSourceStartupStatus,
} from "./input-source-startup-model";
export {
  inputSourceNeedsAdding,
  type InputSourceStartupStatus,
} from "./input-source-startup-model";

export interface InputSourceStartupNoticeProps {
  status: InputSourceStartupStatus;
  onOpenSettings: () => void | Promise<void>;
  onDismiss: () => void;
  onError: (message: string) => void;
}

const settingsPath = "「系统设置 › 键盘 › 文字输入 › 输入法」";
const openSettingsError = `无法打开系统设置，请手动前往${settingsPath}。`;

/** 添加输入源必须由用户在系统设置中完成；macOS 27 的 `TISEnableInputSource` 返回成功但不生效。 */
function AddSteps() {
  return (
    <ol className="m-0 flex list-decimal flex-col gap-1 pl-5 text-[var(--text-secondary)]">
      <li>点「打开键盘设置」，在「文字输入」下的「输入法」一行点「编辑…」。</li>
      <li>点列表左下角的「+」，在左侧选「简体中文」，再选「水杉输入法」，然后点「添加」。</li>
      <li>在菜单栏的输入法菜单里选「水杉输入法」，或按 Control+空格（或地球键）切换过去。</li>
    </ol>
  );
}

/** 显示启动检查结果和用户仍需完成的操作。 */
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
    detail = "请在「维护与诊断」页的「输入法服务」中点「修复」重试。";
  } else if (status.action === "authorization_required") {
    title = "系统安装需要授权更新";
    detail =
      "请在「维护与诊断」页的「输入法服务」中点「修复」，确认管理员授权后更新所有用户的输入法。";
  } else if (status.action === "cancelled") {
    title = "已取消管理员授权";
    detail = "输入法安装未完成。需要时可在「维护与诊断」页的「输入法服务」中点「修复」重试。";
  } else if (status.action === "not_installed") {
    title = "水杉输入法还没有安装到本机";
    detail = "请在「维护与诊断」页的「输入法服务」中点「修复」。";
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
  } else if (status.system_bundles?.length) {
    title = "系统目录里多了一份水杉输入法";
  } else {
    title = "水杉输入法已是当前版本";
  }
  // 只在顺利装好或更新、用户无事可做时配图；失败、要重新登录、要手动加入列表时，页面要把注意力留给接下来的操作。
  const celebrate =
    !failed &&
    !loginRequired &&
    !needsAdding &&
    (status.action === "installed" || status.action === "updated");
  const systemBundles = status.system_bundles?.length
    ? `「/Library/Input Methods」里还有一份水杉输入法（${status.system_bundles.join("、")}），它会让输入法列表出现重复项，或用上较旧的版本。请在「访达」中按 Shift+Command+G 前往该文件夹，把它移到废纸篓（需要管理员密码），然后注销并重新登录。`
    : null;

  return (
    <div
      role={failed ? "alert" : "status"}
      className="section flex flex-col gap-2"
      aria-label="水杉输入法安装状态"
    >
      {celebrate && <MascotIllustration scene="updated" size={72} />}
      <SettingSectionTitle
        as="p"
        className={`section-title m-0${failed ? " text-[var(--danger-text)]" : ""}`}
        title={title}
      />
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
          <ActionButton action={openSettings} className={primary} label="打开键盘设置" />
        )}
        <ActionButton action={onDismiss} label="知道了" />
      </div>
    </div>
  );
}
