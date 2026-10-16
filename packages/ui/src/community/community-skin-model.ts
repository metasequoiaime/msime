import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import type { CommunityModeration, CommunityReportReason } from "./community-report-model";
import type { CommunitySkinCategory } from "./community-skin-category-model";

export type CommunitySkin = {
  id: string;
  name: string;
  description: string;
  author: string;
  design: TouchKeyboardSkinDesign;
  downloads: number;
  rating_count: number;
  rating_average: number;
  owned: boolean;
  my_rating: number;
  /** 只在自己的皮肤中发送；`removed` 显示「已下架」。 */
  moderation?: CommunityModeration | null;
  /** 发布分类；早于分类功能的服务端不返回。 */
  category?: CommunitySkinCategory;
};

export type CommunitySkinPage = {
  skins: CommunitySkin[];
  has_more: boolean;
};

export type CommunitySkinTrial = { id: string; name: string };
export type CommunitySkinDownload = {
  skin: { id: string; name: string; design: TouchKeyboardSkinDesign };
  trial: CommunitySkinTrial;
};

export interface CommunitySkinClient {
  /** `mine` 只列出已登录用户自己的皮肤，包括已下架作品；`category` 为 `null` 时列出全部分类。 */
  list(
    offset: number,
    search: string,
    mine: boolean,
    category: CommunitySkinCategory | null,
  ): Promise<CommunitySkinPage>;
  detail(id: string): Promise<CommunitySkin>;
  download(id: string, name: string): Promise<CommunitySkinDownload>;
  rate(id: string, stars: number): Promise<void>;
  publish(
    id: string,
    name: string,
    description: string,
    design: TouchKeyboardSkinDesign,
    category: CommunitySkinCategory,
  ): Promise<void>;
  unpublish(id: string): Promise<void>;
  /** 修改自己作品的发布分类，返回修改后的条目。 */
  setCategory(id: string, category: CommunitySkinCategory): Promise<CommunitySkin>;
  finishTrial(id: string, keep: boolean): Promise<void>;
  /** 向管理员举报其他用户的皮肤。 */
  report?(id: string, reason: CommunityReportReason, detail: string): Promise<void>;
}
