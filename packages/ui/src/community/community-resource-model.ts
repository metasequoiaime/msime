import { formatZhNumber } from "../core/format-number";
import type { CommunityModeration, CommunityReportReason } from "./community-report-model";

export type CommunityResourceScope = "" | "mine" | "saved";

export type CommunityResourceKind = "dictionary" | "reply";
export type CommunitySharedWord = {
  kind: "pinyin" | "wubi" | "quick" | "english";
  code: string;
  word: string;
  weight: number;
};
export type CommunityResourceContent = { entries?: CommunitySharedWord[]; prompt?: string };
export type CommunityResource = {
  id: string;
  kind: CommunityResourceKind;
  name: string;
  description: string;
  author: string;
  content: CommunityResourceContent;
  revision: number;
  saves: number;
  saved: boolean;
  owned: boolean;
  rating_count: number;
  rating_average: number;
  my_rating: number;
  /** 只在自己的作品里发送；`removed` 显示「已下架」。 */
  moderation?: CommunityModeration | null;
};
export type CommunityResourcePage = { items: CommunityResource[]; has_more: boolean };
export type CommunityResourceApplication = {
  revision: number;
  imported: number;
  resource_revision: number;
};
export type CommunityLocalDictionaryClient = {
  import?(
    kind: "pinyin" | "wubi" | "quick_phrase" | "english",
    format: "standard",
    text: string,
    requestId: string,
  ): Promise<{ applied: number }>;
  /** 宿主有命名词库时，把共享词库装成一个独立的词库（之后可以在「词库」里停用或删除），代替逐条导入本机词库。 */
  installCollection?(resource: CommunityResource): Promise<void>;
};

export interface CommunityResourceClient {
  list(
    kind: CommunityResourceKind,
    scope: CommunityResourceScope,
    search: string,
    offset: number,
  ): Promise<CommunityResourcePage>;
  detail(id: string): Promise<CommunityResource>;
  publish(
    id: string,
    kind: CommunityResourceKind,
    name: string,
    description: string,
    content: CommunityResourceContent,
    revision: number,
  ): Promise<void>;
  apply(id: string, resourceRevision: number): Promise<CommunityResourceApplication>;
  save(id: string, saved: boolean): Promise<void>;
  rate(id: string, stars: number): Promise<void>;
  unpublish(id: string): Promise<void>;
  /** 把回复模板存进本机「高情商回复」键盘、从中移除。只有移动端有这个键盘；桌面宿主不提供这两个，页面就不显示对应的按钮，回复模板只能收藏、评分、举报和发布。 */
  storeReply?(item: CommunityResource): Promise<void>;
  removeReply?(id: string): Promise<void>;
  /** 向管理员举报他人的词库或回复模板。 */
  report?(
    kind: CommunityResourceKind,
    id: string,
    reason: CommunityReportReason,
    detail: string,
  ): Promise<void>;
}

/**
 * HarmonyOS 手机上行下方的元信息行，对应 Android 的 `CommunityRequest.resourceSubtitle`：「@作者 · 4,812 条」。只写条目本身带有的信息：没有作者时不写作者，只有带了词条的词库才写条数，用户自己已下架的作品写「已下架」。设计稿中的「本周更新」需要更新时间，而本客户端的资源不带这个字段，所以从不写出。
 */
export function communityResourceSubtitle(item: CommunityResource): string {
  const entries = item.content.entries;
  return [
    item.author ? `@${item.author}` : "",
    item.kind === "dictionary" && entries && entries.length > 0
      ? `${formatZhNumber(entries.length)} 条`
      : "",
    item.owned && item.moderation === "removed" ? "已下架" : "",
  ]
    .filter(Boolean)
    .join(" · ");
}
