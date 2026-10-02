import { useRef, useState } from "react";
import * as style from "./community-style";
import { CommunitySelectField } from "./community-select-field";
import { ActionButton } from "../core/action-button";

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

/** 画廊上方的分类筛选：「全部」加八个分类。 */
export function CommunitySkinCategoryFilter({
  ariaLabel,
  value,
  onChange,
}: {
  ariaLabel: string;
  value: CommunitySkinCategory | null;
  onChange: (category: CommunitySkinCategory | null) => void;
}) {
  return (
    <div className={style.kindFilter} role="group" aria-label={ariaLabel}>
      <ActionButton
        action={() => onChange(null)}
        className={value === null ? "primary" : "secondary"}
        ariaPressed={value === null}
        label="全部"
      />
      {communitySkinCategories.map((item) => (
        <ActionButton
          key={item}
          action={() => onChange(item)}
          className={value === item ? "primary" : "secondary"}
          ariaPressed={value === item}
          label={communitySkinCategoryLabels[item]}
        />
      ))}
    </div>
  );
}

/** 发布表单和作者修改分类共用的分类下拉框。 */
export function CommunitySkinCategorySelect({
  ariaLabel,
  value,
  disabled,
  onChange,
}: {
  ariaLabel: string;
  value: CommunitySkinCategory;
  disabled: boolean;
  onChange: (category: CommunitySkinCategory) => void;
}) {
  return (
    <CommunitySelectField
      label="分类"
      ariaLabel={ariaLabel}
      value={value}
      disabled={disabled}
      onChange={(nextValue) => onChange(nextValue as CommunitySkinCategory)}
    >
      {communitySkinCategories.map((item) => (
        <option key={item} value={item}>
          {communitySkinCategoryLabels[item]}
        </option>
      ))}
    </CommunitySelectField>
  );
}

/**
 * 画廊的分类筛选状态。分页 hook 只认 offset、搜索词和范围，所以分类放在 `request` 这个 ref 里随列表请求带上，「加载更多」追加的页沿用第一页的分类。
 *
 * `change` 切换分类并用 `reload` 从第一页重新读取；第一页失败时列表和 offset 仍是上一次成功显示的分类的，分类也退回到它，否则「加载更多」会在旧分类的 offset 上追加新分类的页。
 */
export function useCommunitySkinCategoryFilter() {
  const request = useRef<CommunitySkinCategory | null>(null);
  // 只有第一页请求成功后，分类才与当前列表建立关系；请求中的分类不能作为失败回退目标。
  const active = useRef<CommunitySkinCategory | null>(null);
  const [category, setCategory] = useState<CommunitySkinCategory | null>(null);

  const change = async (
    next: CommunitySkinCategory | null,
    reload: () => Promise<boolean>,
  ): Promise<void> => {
    // 与正在请求的分类比较而不是与已显示的比较：请求「自然」途中点回「全部」也要重新读取，否则按钮会停在「自然」。
    if (next === request.current) return;
    const previous = active.current;
    request.current = next;
    setCategory(next);
    const listed = await reload();
    if (request.current !== next) return;
    if (listed) {
      active.current = next;
    } else {
      request.current = previous;
      setCategory(previous);
    }
  };

  return { request, category, change };
}
