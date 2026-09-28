import {
  ScreenKeyboardPreview,
  touchKeyboardSkinOptions,
  type TouchKeyboardSkin,
} from "../keyboard/screen-keyboard-preview";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import * as skin from "../keyboard/touch-skin-style";

export interface ScreenKeyboardSkinsSectionProps {
  mobile: boolean;
  theme: "dark" | "light";
  selected: TouchKeyboardSkin;
  customDesign: TouchKeyboardSkinDesign;
  customAvailable: boolean;
  editorOpen: boolean;
  onSelect: (skin: TouchKeyboardSkin) => void;
  onToggleEditor: () => void;
}

/** Built-in and custom skin choices for the shared screen keyboard. */
export function ScreenKeyboardSkinsSection({
  mobile,
  theme,
  selected,
  customDesign,
  customAvailable,
  editorOpen,
  onSelect,
  onToggleEditor,
}: ScreenKeyboardSkinsSectionProps) {
  return (
    <div className="section" role="group" aria-labelledby="touch-keyboard-skin-title">
      <div className="section-title" id="touch-keyboard-skin-title">
        键盘皮肤
        <small>与 Apple 内置皮肤一致；独立于{mobile ? "候选栏" : "桌面候选窗"}皮肤</small>
      </div>
      <div className={skin.skinGrid}>
        {touchKeyboardSkinOptions.map((option) => (
          <article className={skin.skinCard(selected === option.id)} key={option.id}>
            <button
              type="button"
              className={skin.skinCardButton}
              role="switch"
              aria-label={`屏幕键盘皮肤 ${option.title}`}
              aria-checked={selected === option.id}
              onClick={() => onSelect(option.id)}
            >
              <ScreenKeyboardPreview theme={theme} skin={option.id} compact />
              <span className={skin.skinCardCopy}>
                <strong>{option.title}</strong>
                <small>{option.description}</small>
              </span>
              <span className={skin.skinCardCheck} aria-hidden="true">
                {selected === option.id ? "✓" : ""}
              </span>
            </button>
          </article>
        ))}
        {customAvailable && (
          <article className={skin.skinCard(selected === "custom")}>
            <button
              type="button"
              className={skin.skinCardButton}
              role="switch"
              aria-label="屏幕键盘皮肤 我的皮肤"
              aria-checked={selected === "custom"}
              onClick={() => onSelect("custom")}
            >
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
                {selected === "custom" ? "✓" : ""}
              </span>
            </button>
          </article>
        )}
      </div>
      {customAvailable && (
        <button
          type="button"
          className={`secondary ${skin.editorOpen}`}
          aria-expanded={editorOpen}
          onClick={onToggleEditor}
        >
          {editorOpen ? "收起自定义编辑器" : "设计我的皮肤"}
        </button>
      )}
    </div>
  );
}
