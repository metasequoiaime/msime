import * as style from "./community-style";

export function CommunityBackButton({
  disabled,
  onClick,
}: {
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={style.back}
      disabled={disabled}
      onClick={onClick}
      aria-label="返回社区"
    >
      ← 社区
    </button>
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
    <button type="button" className={`secondary ${style.more}`} disabled={disabled} onClick={onClick}>
      加载更多
    </button>
  );
}
