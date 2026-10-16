/** 社区皮肤的发布分类，键盘皮肤和候选窗口皮肤共用，与服务端的固定 id 一致；只是发布元数据，不写进皮肤内容。 */
export type CommunitySkinCategory =
  | "nature"
  | "guofeng"
  | "acg"
  | "cute"
  | "food"
  | "tech"
  | "minimal"
  | "other";

/** 全部分类，顺序即筛选按钮和发布表单选项的顺序。 */
export const communitySkinCategories: readonly CommunitySkinCategory[] = [
  "nature",
  "guofeng",
  "acg",
  "cute",
  "food",
  "tech",
  "minimal",
  "other",
];

export const communitySkinCategoryLabels: Record<CommunitySkinCategory, string> = {
  nature: "自然",
  guofeng: "国风",
  acg: "二次元",
  cute: "可爱",
  food: "美食",
  tech: "科技夜色",
  minimal: "简约",
  other: "其他",
};

/** 条目的分类名称；早于分类功能的服务端不返回分类，此时为 `""`。宿主已把未知的新分类读作 `other`。 */
export function communitySkinCategoryLabel(category: CommunitySkinCategory | undefined): string {
  return category ? (communitySkinCategoryLabels[category] ?? "") : "";
}
