import { ActionButton } from "../core/action-button";

export interface CloudPanelHeaderProps {
  title: string;
  onClose: () => void;
  onBack?: () => void;
  backClassName?: string;
}

/** Shared title bar for the cloud panels, with an optional return action. */
export function CloudPanelHeader({ title, onClose, onBack, backClassName }: CloudPanelHeaderProps) {
  return (
    <header className="native-panel-header">
      {onBack && (
        <ActionButton action={onBack} ariaLabel="返回云词库" className={backClassName} label="‹" />
      )}
      <span>{title}</span>
      <ActionButton action={onClose} ariaLabel="关闭" label="×" />
    </header>
  );
}
