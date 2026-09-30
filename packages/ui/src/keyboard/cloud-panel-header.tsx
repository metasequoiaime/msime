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
        <button className={backClassName} type="button" aria-label="返回云词典" onClick={onBack}>
          ‹
        </button>
      )}
      <span>{title}</span>
      <button type="button" aria-label="关闭" onClick={onClose}>
        ×
      </button>
    </header>
  );
}
