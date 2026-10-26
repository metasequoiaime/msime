import type { PluginKind, PluginPackage } from "../settings/plugin-types";
import type { CommunityModeration, CommunityReportReason } from "./community-report-model";

/** 包可以按哪些类型分享：八种数据类插件各平台都能分享，桌面宿主另有脚本插件。与 `client-services::plugins::community::PUBLISHABLE_KINDS` 保持一致。 */
export type CommunityPluginKind = PluginKind;

/** 各平台都能分享的数据类插件。 */
export const communityPluginKinds: readonly CommunityPluginKind[] = [
  "sound",
  "music",
  "command_table",
  "effect",
  "phrase_table",
  "helpcode",
  "wordbook",
  "symbol_set",
];

/** 这个宿主能分享的类型：运行脚本插件的桌面宿主（`HostCapabilities.script_plugins`）多一种 `script`。 */
export function communityPluginKindsFor(scriptPlugins: boolean): readonly CommunityPluginKind[] {
  return scriptPlugins ? [...communityPluginKinds, "script"] : communityPluginKinds;
}

/** 脚本插件在社区里先审后发：作者自己的脚本插件审核通过前是 `pending`，显示「审核中」，别人看不到它。数据类插件发布即公开，`pending` 当作已通过，不显示任何状态。 */
export function communityPluginUnderReview(plugin: CommunityPlugin): boolean {
  return plugin.kind === "script" && plugin.owned && plugin.moderation === "pending";
}

/** 一条发布，对应 `client-services::plugins::community::CommunityPlugin`。 */
export type CommunityPlugin = {
  id: string;
  kind: CommunityPluginKind;
  /** 清单标识，同时也是插件包的安装目录名。 */
  plugin_id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  license: string;
  size: number;
  sha256: string;
  downloads: number;
  rating_count: number;
  rating_average: number;
  owned: boolean;
  my_rating: number;
  created_at: string;
  /** 只在自己的插件包中发送；`removed` 显示「已下架」，脚本插件的 `pending` 显示「审核中」（见 `communityPluginUnderReview`）。 */
  moderation?: CommunityModeration | null;
  /** 脚本插件清单申请的能力名，按清单顺序；数据类插件没有。可能有本客户端还不认识的名字。 */
  permissions?: string[];
};

export type CommunityPluginPage = {
  plugins: CommunityPlugin[];
  has_more: boolean;
  /** 当前客户端无法安装的新类型包，由宿主过滤，但仍计入下一页偏移量。 */
  skipped?: number;
};

/** 宿主打包器在上传前读取的已安装包信息。 */
export type CommunityPluginPackPreview = {
  suggestedName: string;
  suggestedDescription: string;
  version: string;
  license: string;
  fileCount: number;
  size: number;
};

/** 插件包的桌面社区命令。宿主自行打包已安装包，下载后沿用本机 `.zip` 的导入流程；WebView 只传包的类型与标识或发布标识，不传路径和字节。 */
export interface CommunityPluginClient {
  list(
    offset: number,
    search: string,
    kind: CommunityPluginKind | null,
    /** 只列出已登录用户自己的包，包括已下架作品。 */
    mine?: boolean,
  ): Promise<CommunityPluginPage>;
  detail(id: string): Promise<CommunityPlugin>;
  packPreview(kind: CommunityPluginKind, pluginId: string): Promise<CommunityPluginPackPreview>;
  /** `id` 是页面的发布标识：失败后复用它，服务端可在重试时返回已保存的作品。 */
  publish(
    kind: CommunityPluginKind,
    pluginId: string,
    id: string,
    name: string,
    description: string,
  ): Promise<CommunityPlugin>;
  /** 下载、校验并替换同类型同标识的已安装包。`kind` 和 `pluginId` 来自页面请求替换的条目；下载包标识不符时宿主拒绝安装。 */
  install(id: string, kind: CommunityPluginKind, pluginId: string): Promise<PluginPackage>;
  rate(id: string, stars: number): Promise<{ stars: number }>;
  delete(id: string): Promise<{ deleted: boolean }>;
  /** 向管理员举报其他用户的插件包。 */
  report?(id: string, reason: CommunityReportReason, detail: string): Promise<void>;
}
