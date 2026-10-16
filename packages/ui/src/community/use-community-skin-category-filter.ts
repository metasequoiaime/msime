import { useRef, useState } from "react";
import type { CommunitySkinCategory } from "./community-skin-category-model";

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
