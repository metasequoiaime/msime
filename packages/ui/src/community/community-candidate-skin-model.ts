import type { SkinCatalog } from "../skin/external-skins";
import type { CommunityModeration, CommunityReportReason } from "./community-report-model";
import {
  communitySkinCategories,
  communitySkinCategoryLabels,
  type CommunitySkinCategory,
} from "./community-skin-category-model";

/** 服务端的许可证字段；清单缺省时各字段为 `""`。 */
export type CommunityCandidateSkinLicense = {
  code: string;
  assets: string;
  source: string;
};

/** 包的可见范围：所有图库用户或仅作者本人；作者的库同步保留私有包。 */
export type CandidateSkinVisibility = "public" | "private";

/** 候选窗口皮肤的发布分类，与社区键盘皮肤共用；只是发布元数据，不写进 skin.toml。 */
export type CandidateSkinCategory = CommunitySkinCategory;
export const candidateSkinCategories = communitySkinCategories;
export const candidateSkinCategoryLabels = communitySkinCategoryLabels;

export type CommunityCandidateSkin = {
  id: string;
  /** 清单标识，同时也是包的安装目录名。 */
  package_id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  license: CommunityCandidateSkinLicense;
  size: number;
  file_count: number;
  downloads: number;
  rating_count: number;
  rating_average: number;
  owned: boolean;
  my_rating: number;
  created_at: string;
  visibility: CandidateSkinVisibility;
  updated_at: string;
  /** 只在自己的包中发送；`removed` 显示「已下架」。 */
  moderation?: CommunityModeration | null;
  /** 发布分类；早于分类功能的服务端不返回。 */
  category?: CandidateSkinCategory;
};

export type CommunityCandidateSkinPage = {
  skins: CommunityCandidateSkin[];
  has_more: boolean;
};

/** 宿主打包器在上传前读取的本机包信息；许可证来自 `skin.toml`，缺省字段为 `null`。 */
export type CandidateSkinPackPreview = {
  suggestedName: string;
  license: {
    code: string | null;
    assets: string | null;
    source: string | null;
  };
  fileCount: number;
  size: number;
};

/** 一次同步中保持原样的包，附带 `candidate_skin_*` 规则码或 `account_*` 请求码。 */
export type CandidateSkinSyncSkip = { package_id: string; code: string };

/** 本机皮肤目录与用户库的一次同步结果，各列表按包标识记录。 */
export type CandidateSkinSyncReport = {
  uploaded: string[];
  downloaded: string[];
  deleted_local: string[];
  deleted_cloud: string[];
  skipped: CandidateSkinSyncSkip[];
  /** 本次后续上传停止时为 `account_rate_limited` 或 `candidate_skin_library_limit`。 */
  stopped: string | null;
};

/** 候选窗口皮肤包的桌面社区命令；宿主负责打包、下载和安装，WebView 不处理路径或包字节。 */
export interface CandidateSkinCommunityClient {
  /** `category` 为 `null` 时列出全部分类。 */
  list(
    offset: number,
    search: string,
    mine: boolean,
    category: CandidateSkinCategory | null,
  ): Promise<CommunityCandidateSkinPage>;
  detail(id: string): Promise<CommunityCandidateSkin>;
  preview(id: string): Promise<{ dataUrl: string }>;
  /** 下载并安装到外部皮肤目录，返回重新扫描的目录。 */
  install(id: string, replace: boolean): Promise<SkinCatalog>;
  /** 按 `visibility` 校验包：只有公开包需要素材许可证。 */
  packPreview(
    skinId: string,
    visibility: CandidateSkinVisibility,
  ): Promise<CandidateSkinPackPreview>;
  /** 把页面绘制的 PNG 或 JPEG 预览 `bytes` 写入尚无预览的已安装包 `skinId`，返回重扫目录。 */
  addPreview(skinId: string, bytes: number[]): Promise<SkinCatalog>;
  /** 把用户选择的素材许可证 `assets` 写入尚无此项的已安装包 `skinId` 清单，返回重扫目录。 */
  addLicense(skinId: string, assets: string): Promise<SkinCatalog>;
  /** 同步已保存在库中的包原地更新，发布不会留下第二份副本。 */
  publish(
    skinId: string,
    id: string,
    name: string,
    description: string,
    visibility: CandidateSkinVisibility,
    category: CandidateSkinCategory,
  ): Promise<CommunityCandidateSkin>;
  setVisibility(id: string, visibility: CandidateSkinVisibility): Promise<CommunityCandidateSkin>;
  /** 修改自己作品的发布分类。 */
  setCategory(id: string, category: CandidateSkinCategory): Promise<CommunityCandidateSkin>;
  /** 同步本机皮肤目录与已登录用户的库；未登录时为 `community_unauthorized`。 */
  sync(): Promise<CandidateSkinSyncReport>;
  rate(id: string, stars: number): Promise<{ stars: number }>;
  unpublish(id: string): Promise<{ deleted: boolean }>;
  /** 向管理员举报其他用户的包。 */
  report?(id: string, reason: CommunityReportReason, detail: string): Promise<void>;
}
