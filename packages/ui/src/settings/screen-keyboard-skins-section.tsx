import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import * as skin from "../keyboard/touch-skin-style";
import { ActionButton } from "../core/action-button";
import { SettingsGroupBlock } from "./settings-group-block";

export interface ScreenKeyboardSkinsSectionProps {
  theme: "dark" | "light";
  /** Whether the custom theme draws the keyboard with 我的皮肤 rather than its base theme's keyboard. */
  selected: boolean;
  customDesign: TouchKeyboardSkinDesign;
  editorOpen: boolean;
  onSelect: () => void;
  onToggleEditor: () => void;
}

/** 我的皮肤, the custom keyboard skin that is part of the custom theme, and the switch for its editor. The built-in keyboard looks are the global themes themselves. */
export function ScreenKeyboardSkinsSection({
  theme,
  selected,
  customDesign,
  editorOpen,
  onSelect,
  onToggleEditor,
}: ScreenKeyboardSkinsSectionProps) {
  return (
    <SettingsGroupBlock role="group" aria-label="我的皮肤">
      <div className={skin.skinGrid}>
        <article className={skin.skinCard(selected)}>
          <ActionButton
            action={onSelect}
            className={skin.skinCardButton}
            role="switch"
            ariaLabel="屏幕键盘皮肤 我的皮肤"
            ariaChecked={selected}
            label={
              <>
                <ScreenKeyboardPreview
                  theme={theme}
                  skin="custom"
                  customDesign={customDesign}
                  compact
                />
                <span className={skin.skinCardCopy}>
                  <strong>我的皮肤</strong>
                  <small>自由配色 · 自定义键帽</small>
                </span>
                <span className={skin.skinCardCheck} aria-hidden="true">
                  {selected ? "✓" : ""}
                </span>
              </>
            }
          />
        </article>
      </div>
      <ActionButton
        action={onToggleEditor}
        ariaExpanded={editorOpen}
        className={`secondary ${skin.editorOpen}`}
        label={editorOpen ? "收起自定义编辑器" : "设计我的皮肤"}
      />
    </SettingsGroupBlock>
  );
}
