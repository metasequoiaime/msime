import { expect, test } from "vitest";
import {
  defaultTouchKeyboardSkinDesign,
  hasReadableSkinText,
  skinContrast,
  touchKeyboardBackgroundPresets,
} from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

// 预设带一整套配色：按键文字在按键上可读、强调色在背景和按键上可读，按键文字色压在背景（渐变两端）上也至少 3:1，工具栏图标和候选栏文字才看得见。
test("every background preset keeps its text readable on the keys and on the background", () => {
  for (const preset of touchKeyboardBackgroundPresets) {
    const design = {
      ...defaultTouchKeyboardSkinDesign,
      background: preset.start,
      gradientEnd: preset.end,
      keyBackground: preset.keyBackground,
      keyForeground: preset.keyForeground,
      accent: preset.accent,
      actionBackground: preset.actionBackground,
    };
    expect(hasReadableSkinText(design), preset.title).toBe(true);
    for (const background of [preset.start, preset.end ?? preset.start])
      expect(skinContrast(preset.keyForeground, background), preset.title).toBeGreaterThanOrEqual(
        3,
      );
  }
});
