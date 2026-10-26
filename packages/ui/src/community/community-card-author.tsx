import * as style from "./community-style";

export interface CommunityCardAuthorProps {
  prefix?: string;
  author: string;
  owned: boolean;
  private?: boolean;
  removed?: boolean;
  /** 先审后发的作品（脚本插件）还在审核，只有作者看得到。 */
  reviewing?: boolean;
}

/** Shared author line for community gallery cards. */
export function CommunityCardAuthor({
  prefix,
  author,
  owned,
  private: isPrivate = false,
  removed = false,
  reviewing = false,
}: CommunityCardAuthorProps) {
  return (
    <span className={style.cardAuthor}>
      {prefix && `${prefix} · `}
      {owned ? "我的作品" : author}
      {isPrivate && " · 私有"}
      {owned && removed && " · 已下架"}
      {owned && !removed && reviewing && " · 审核中"}
    </span>
  );
}
