import type { PointerEvent as ReactPointerEvent } from "react";
import { ScreenKeyboardPreview, type TouchKeyboardSkin } from "../keyboard/screen-keyboard-preview";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import type { PreviewTheme } from "../skin/skin-preview-palette";
import * as settings from "./settings-style";

export interface ScreenKeyboardLaunchSectionProps {
  openScreenKeyboard?: () => void | Promise<void>;
  theme: PreviewTheme;
  skin: TouchKeyboardSkin;
  customDesign?: TouchKeyboardSkinDesign;
  keySpacingTenths: number;
  rowSpacingTenths: number;
  heightAdjustment: number;
  onPointerDown: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerMove: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerUp: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerCancel: (event: ReactPointerEvent<HTMLDivElement>) => void;
}

/** Launches the screen keyboard and keeps its drag-adjustable preview with the launcher. */
export function ScreenKeyboardLaunchSection({
  openScreenKeyboard,
  theme,
  skin,
  customDesign,
  keySpacingTenths,
  rowSpacingTenths,
  heightAdjustment,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
}: ScreenKeyboardLaunchSectionProps) {
  return (
    <div className={`section ${settings.launchCard}`}>
      <div className={`section-header ${settings.launchRow}`}>
        <span className="section-title">
          打开屏幕键盘<small>使用鼠标或触控方式输入文字与快捷按键</small>
        </span>
        <button
          type="button"
          className={`secondary ${settings.openButton}`}
          disabled={!openScreenKeyboard}
          onClick={() => void openScreenKeyboard?.()}
        >
          打开
        </button>
      </div>
      <div className={settings.panelPreview} aria-label="屏幕键盘预览">
        <div className={settings.panelPreviewLabel}>预览</div>
        <div
          aria-label="拖动预览调整键盘间距"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerCancel}
          style={{ touchAction: "none" }}
        >
          <ScreenKeyboardPreview
            theme={theme}
            skin={skin}
            customDesign={customDesign}
            keySpacingTenths={keySpacingTenths}
            rowSpacingTenths={rowSpacingTenths}
            heightAdjustment={heightAdjustment}
          />
        </div>
      </div>
    </div>
  );
}
