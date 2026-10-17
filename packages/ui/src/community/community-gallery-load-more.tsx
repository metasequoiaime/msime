import { CommunityLoadMoreButton } from "./community-gallery-controls";
import { StatusMessage } from "../core/status-message";
import * as style from "./community-style";

export interface CommunityGalleryLoadMoreProps {
  hasMore: boolean;
  busy: boolean;
  loadingText: string;
  onLoadMore: () => void;
  /** 鸿蒙手机用轻量文字按钮和提示；缺省保留桌面外观。 */
  look?: "harmony";
}

/** 社区图库共用的分页按钮与加载提示（转圈加文字），两种外观共用显示和禁用条件；首屏读取和加载更多都画在这里。 */
export function CommunityGalleryLoadMore({
  hasMore,
  busy,
  loadingText,
  onLoadMore,
  look,
}: CommunityGalleryLoadMoreProps) {
  const harmony = look === "harmony";
  if (!hasMore && !busy) return null;
  return (
    <>
      {hasMore && (
        <CommunityLoadMoreButton
          className={harmony ? style.harmonyMore : undefined}
          disabled={busy}
          onClick={onLoadMore}
        />
      )}
      {busy && (
        <StatusMessage
          role="status"
          className={`${harmony ? style.harmonyNotice : style.notice} ${style.loadingNotice}`}
        >
          <span className={style.loadingSpinner} aria-hidden="true" />
          {loadingText}
        </StatusMessage>
      )}
    </>
  );
}

/** 兼容原鸿蒙分页入口，逻辑统一由公共分页控件持有。 */
export function CommunityHarmonyLoadMore(props: Omit<CommunityGalleryLoadMoreProps, "look">) {
  return <CommunityGalleryLoadMore {...props} look="harmony" />;
}
