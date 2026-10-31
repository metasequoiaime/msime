import type { CommunityReportKind } from "./community-report-model";

/** 屏蔽作者时说的是哪类作品，写法与举报相同（`POST /v1/community/blocks` 的 `kind`）：服务端按这件作品找到作者，客户端从不知道作者的账号。 */
export type CommunityBlockKind = CommunityReportKind;

/** 一位已屏蔽的作者：`id` 是这条屏蔽的编号（取消屏蔽用它，不是作者的账号），`name` 是作者现在的显示名，`created_at` 是 RFC 3339 时间。与 client-services 的 `CommunityBlock` 相同。 */
export interface CommunityBlock {
  id: string;
  name: string;
  created_at: string;
}

/**
 * 屏蔽作者：Rust `msime_client_services::community::block` 的三个操作，宿主原样转交。三项都只认登录的水杉账号，不退回本机匿名账号，没有登录时以 `community_unauthorized` 拒绝。屏蔽之后服务端不再在这个账号的社区列表里列出那位作者的作品（「我的作品」除外），客户端只需重读列表。
 *
 * 宿主不提供它时，作品详情里没有「屏蔽作者」，隐私页也没有「已屏蔽的作者」。
 */
export interface CommunityBlockClient {
  block(kind: CommunityBlockKind, id: string): Promise<CommunityBlock>;
  /** 最近屏蔽的在前。 */
  list(): Promise<CommunityBlock[]>;
  /** `id` 是屏蔽记录的编号；已经取消过的也按成功处理。 */
  unblock(id: string): Promise<void>;
}

/** 一个账号最多屏蔽的作者数，与服务端的上限（client-services 的 `MAX_COMMUNITY_BLOCKS`）相同。 */
export const communityBlockLimit = 500;

/** 没有登录时点「屏蔽作者」看到的提示，下面跟着「去登录」。与 iOS、Android 同一句。 */
export const communityBlockSignInMessage = "登录水杉账号后才能屏蔽作者。";

export function communityBlockConfirmTitle(author: string): string {
  return `屏蔽作者「${author}」？`;
}

/** 确认屏蔽时的说明。取消屏蔽的位置分宿主：触屏宿主在「我的 › 隐私」，这句与 iOS、Android 相同；桌面和 2in1 没有那一页，「已屏蔽的作者」在「关于 › 许可与隐私」里。 */
export function communityBlockConfirmMessage(mobile: boolean): string {
  const place = mobile ? "我的 › 隐私 › 已屏蔽的作者" : "关于 › 许可与隐私 › 已屏蔽的作者";
  return `屏蔽后，社区里不再显示这位作者发布的皮肤、插件、词库和模板。可以在「${place}」里取消屏蔽。`;
}

export function communityBlockedNotice(author: string): string {
  return `已屏蔽「${author}」`;
}

export const communityBlockedAuthorsTitle = "已屏蔽的作者";

export const communityBlockedAuthorsEmpty =
  "没有屏蔽任何作者。在社区作品详情里点「屏蔽作者」，就不会再看到这位作者的作品。";

export const communityBlockedAuthorsLoadError = "暂时无法读取已屏蔽的作者，请重试。";

/** 屏蔽的日期，按本机时区写成 yyyy-MM-dd；读不出时间时为空字符串。 */
export function communityBlockDay(createdAt: string): string {
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}
