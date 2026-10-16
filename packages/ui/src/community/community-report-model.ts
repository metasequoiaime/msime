/** 举报针对的对象类别，写法与 `POST /v1/community/reports` 和 client-services 的 `CommunityReportKind` 一致。 */
export type CommunityReportKind =
  | "skins"
  | "candidate-skins"
  | "plugins"
  | "dictionaries"
  | "replies";

/** 举报可选的固定理由，按对话框列出的顺序；标签就是审核人员看到的文字。与 client-services 的 `CommunityReportReason::ALL` 对应。 */
export const communityReportReasons = [
  "侵权/抄袭",
  "色情低俗",
  "违法违规",
  "垃圾广告",
  "恶意插件",
  "其他",
] as const;

export type CommunityReportReason = (typeof communityReportReasons)[number];

/** 服务端接受的补充说明长度上限。 */
export const communityReportDetailLimit = 1000;

/** 已登录用户自己作品的审核状态；只在自己的作品中发送。 */
export type CommunityModeration = "approved" | "pending" | "removed";

/** 图库提交举报后显示的提示；重复举报同一作品也按成功处理。 */
export const communityReportedNotice = "已收到举报，我们会尽快处理。";
