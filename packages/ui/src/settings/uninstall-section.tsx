import * as settings from "./settings-style";
import { GroupList } from "../core/platform-controls";

export interface UninstallSectionProps {
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
  removeUserData: boolean;
  uninstallBusy: boolean;
  uninstallConfirmation: boolean;
  uninstallResult: "success" | "error" | null;
  onRemoveUserDataChange: (value: boolean) => void;
  onRequestUninstall: () => void;
  onConfirmUninstall: () => void;
  onCancelUninstall: () => void;
}

/** macOS 输入源移除，「维护与诊断」的最后一组。不能卸载的宿主什么都不画。 */
export function UninstallSection({
  uninstallInputSource,
  removeUserData,
  uninstallBusy,
  uninstallConfirmation,
  uninstallResult,
  onRemoveUserDataChange,
  onRequestUninstall,
  onConfirmUninstall,
  onCancelUninstall,
}: UninstallSectionProps) {
  if (!uninstallInputSource) return null;
  return (
    <GroupList title="卸载">
      <div className={settings.rowStack} role="group" aria-label="卸载">
        <div
          className={`${settings.managerBlock} ${settings.serviceRow} ${settings.serviceRowDanger}`}
        >
          <span>
            卸载水杉输入法
            <small>输入源会移到废纸篓；默认保留词库、学习记录和偏好，重新安装后可继续使用。</small>
            <label>
              <input
                type="checkbox"
                checked={removeUserData}
                onChange={(event) => onRemoveUserDataChange(event.target.checked)}
              />{" "}
              同时删除词库、偏好与语音密钥
            </label>
          </span>
          <div>
            <button
              type="button"
              className="secondary"
              disabled={uninstallBusy}
              aria-label="卸载…"
              aria-busy={uninstallBusy}
              onClick={onRequestUninstall}
            >
              {uninstallBusy ? "处理中…" : "卸载…"}
            </button>
            {uninstallResult === "success" && <span role="status">输入法已移到废纸篓。</span>}
            {uninstallResult === "error" && <span role="alert">卸载未能完成，请稍后重试。</span>}
          </div>
          {uninstallConfirmation && (
            <div
              className={settings.serviceConfirmation}
              role="alertdialog"
              aria-modal="true"
              aria-label="确认卸载水杉输入法"
            >
              <p>
                输入法会被移到废纸篓，放错了可以从那里放回原处。
                {removeUserData
                  ? "已选择同时删除词库、偏好与语音密钥。"
                  : "词库、学习记录和偏好会保留，重新安装后可以继续使用。"}{" "}
                卸载后请重新登录系统，让它从输入源列表中消失。
              </p>
              <div>
                <button
                  type="button"
                  className="danger"
                  disabled={uninstallBusy}
                  onClick={onConfirmUninstall}
                >
                  确认卸载
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={uninstallBusy}
                  onClick={onCancelUninstall}
                >
                  取消
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </GroupList>
  );
}
