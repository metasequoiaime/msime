import * as style from "./community-style";
import { CommunitySelectField } from "./community-select-field";
import { ActionButton } from "../core/action-button";

import {
  type CommunitySkinCategory,
  communitySkinCategories,
  communitySkinCategoryLabels,
} from "./community-skin-category-model";
export {
  type CommunitySkinCategory,
  communitySkinCategories,
  communitySkinCategoryLabels,
  communitySkinCategoryLabel,
} from "./community-skin-category-model";

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

export { useCommunitySkinCategoryFilter } from "./use-community-skin-category-filter";
