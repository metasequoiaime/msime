import type { EmojiCatalogItem } from "../emoji/emoji-catalog";
import { emojiDisplayName } from "./emoji-panel-helpers";
import { panelTile } from "./emoji-panel-style";

export interface EmojiItemButtonProps {
  item: EmojiCatalogItem;
  onSelect: (item: EmojiCatalogItem) => void;
  flow?: boolean;
  compact?: boolean;
  navigable?: boolean;
  disabled?: boolean;
  className?: string;
}

/** 表情与符号共用的受控条目，几何、导航和禁用规则由调用方提供。 */
export function EmojiItemButton({
  item,
  onSelect,
  flow = false,
  compact = false,
  navigable = false,
  disabled,
  className,
}: EmojiItemButtonProps) {
  const tile = panelTile(flow, compact);
  return (
    <button
      type="button"
      className={className ? `${tile} ${className}` : tile}
      data-emoji-navigation-item={navigable ? true : undefined}
      disabled={disabled}
      title={emojiDisplayName(item.keywords, item.text)}
      onClick={() => onSelect(item)}
    >
      {item.text}
    </button>
  );
}
