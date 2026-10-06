import * as style from "./community-style";

export interface CommunityCardAuthorProps {
  prefix?: string;
  author: string;
  owned: boolean;
  private?: boolean;
  removed?: boolean;
}

/** Shared author line for community gallery cards. */
export function CommunityCardAuthor({
  prefix,
  author,
  owned,
  private: isPrivate = false,
  removed = false,
}: CommunityCardAuthorProps) {
  return (
    <span className={style.cardAuthor}>
      {prefix && `${prefix} · `}
      {owned ? "我的作品" : author}
      {isPrivate && " · 私有"}
      {owned && removed && " · 已下架"}
    </span>
  );
}
