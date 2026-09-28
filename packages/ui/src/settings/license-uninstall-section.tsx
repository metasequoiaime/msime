import * as doc from "./document-style";
import * as settings from "./settings-style";

export interface LicenseUninstallSectionProps {
  openThirdPartyLicenses?: () => Promise<void>;
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

/** macOS licensing, third-party notices, and input-source removal controls. */
export function LicenseUninstallSection({
  openThirdPartyLicenses,
  uninstallInputSource,
  removeUserData,
  uninstallBusy,
  uninstallConfirmation,
  uninstallResult,
  onRemoveUserDataChange,
  onRequestUninstall,
  onConfirmUninstall,
  onCancelUninstall,
}: LicenseUninstallSectionProps) {
  return (
    <div className="section" role="group" aria-label="许可与卸载">
      <div className="section-header">
        <span className="section-title">
          许可与版权
          <small>水杉 IME 以 GPL-3.0 发布；第三方组件许可随应用资源提供。</small>
        </span>
        <span className={doc.version} aria-label="版权">
          © 2026 Metasequoia IME
        </span>
      </div>
      {openThirdPartyLicenses && (
        <button type="button" className="secondary" onClick={() => void openThirdPartyLicenses()}>
          查看许可全文
        </button>
      )}
      {uninstallInputSource && (
        <div className={`${settings.serviceRow} ${settings.serviceRowDanger}`}>
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
      )}
    </div>
  );
}
