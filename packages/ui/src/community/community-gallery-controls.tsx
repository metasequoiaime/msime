import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

export function CommunityBackButton({
  disabled,
  onClick,
  ariaLabel = "返回社区",
}: {
  disabled: boolean;
  onClick: () => void;
  ariaLabel?: string;
}) {
  return (
    <ActionButton
      action={onClick}
      className={style.back}
      disabled={disabled}
      ariaLabel={ariaLabel}
      label="← 社区"
    />
  );
}

export function CommunityLoadMoreButton({
  disabled,
  onClick,
  className = `secondary ${style.more}`,
}: {
  disabled: boolean;
  onClick: () => void;
  className?: string;
}) {
  return (
    <ActionButton action={onClick} className={className} disabled={disabled} label="加载更多" />
  );
}
