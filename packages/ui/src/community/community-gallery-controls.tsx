import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

export function CommunityBackButton({
  disabled,
  onClick,
}: {
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <ActionButton
      action={onClick}
      className={style.back}
      disabled={disabled}
      ariaLabel="返回社区"
      label="← 社区"
    />
  );
}

export function CommunityLoadMoreButton({
  disabled,
  onClick,
}: {
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <ActionButton
      action={onClick}
      className={`secondary ${style.more}`}
      disabled={disabled}
      label="加载更多"
    />
  );
}
