import { ActionButton } from "../core/action-button";
import { StatusMessage } from "../core/status-message";

export function SettingsStartupPage({ onClose }: { onClose?: () => void }) {
  return (
    <main className="settings-startup" aria-label="设置加载中" aria-busy="true">
      <div className="settings-startup-spinner" aria-hidden="true" />
      <div className="settings-startup-dots" aria-hidden="true">
        <i />
        <i />
        <i />
      </div>
      <h1>正在打开设置</h1>
      <StatusMessage role="status">冷启动可能需要稍等片刻</StatusMessage>
      {onClose && (
        <ActionButton
          action={onClose}
          className="settings-startup-close"
          ariaLabel="关闭设置"
          label="×"
        />
      )}
    </main>
  );
}
