import type { CommunityDestinationCategory } from "./community-destination";
import * as style from "./community-style";

export interface CommunityCategoryTabsProps {
  category: CommunityDestinationCategory;
  hasSkins: boolean;
  look?: "harmony";
  onCategoryChange: (category: CommunityDestinationCategory) => void;
}

/** 社区首页的受控分类切换，按宿主能力隐藏皮肤分类。 */
export function CommunityCategoryTabs({
  category,
  hasSkins,
  look,
  onCategoryChange,
}: CommunityCategoryTabsProps) {
  const harmony = look === "harmony";
  // 设计稿把第三段命名为「短语」。本宿主没有接入短语包，所以在 HarmonyOS 上这一段放的是回复模板，使用它们自己的分区标题。
  const tabs: { category: CommunityDestinationCategory; label: string }[] = [
    ...(hasSkins ? [{ category: "skin" as const, label: "皮肤" }] : []),
    { category: "dictionary", label: "词库" },
    { category: "reply", label: harmony ? "短语" : "回复模板" },
  ];
  return (
    <div
      className={harmony ? style.harmonySegments : style.categoryTabs}
      role="tablist"
      aria-label="社区分类"
    >
      {tabs.map((tab) => (
        <button
          key={tab.category}
          type="button"
          role="tab"
          aria-selected={category === tab.category}
          onClick={() => onCategoryChange(tab.category)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
