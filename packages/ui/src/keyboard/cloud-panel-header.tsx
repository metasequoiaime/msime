import { ActionButton } from "../core/action-button";
import type { PanelClient } from "./panel-client";
import { usePanelDrag } from "./use-panel-drag";

export interface CloudPanelHeaderProps {
  title: string;
  onClose: () => void;
  client?: Pick<PanelClient, "beginWindowDrag">;
  onBack?: () => void;
  backClassName?: string;
}

/** 云面板共用标题栏，可选返回操作。 */
export function CloudPanelHeader({
  title,
  onClose,
  client,
  onBack,
  backClassName,
}: CloudPanelHeaderProps) {
  const drag = usePanelDrag(client ?? {}, () => undefined);
  return (
    <header className="native-panel-header" {...drag}>
      {onBack && (
        <ActionButton action={onBack} ariaLabel="返回云词库" className={backClassName} label="‹" />
      )}
      <span>{title}</span>
      <ActionButton action={onClose} ariaLabel="关闭" label="×" />
    </header>
  );
}
