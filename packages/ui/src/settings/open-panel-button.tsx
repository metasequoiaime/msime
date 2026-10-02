import { ActionButton } from "./action-button";

export interface OpenPanelButtonProps {
  action?: () => void | Promise<void>;
  label?: string;
  className?: string;
}

/** Shared button for opening an optional settings panel or native surface. */
export function OpenPanelButton({
  action,
  label = "打开",
  className = "secondary",
}: OpenPanelButtonProps) {
  return <ActionButton action={action} label={label} className={className} />;
}
