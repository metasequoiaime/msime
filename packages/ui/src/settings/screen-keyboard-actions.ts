import type { Dispatch, SetStateAction } from "react";
import type { TouchKeyboardSkin } from "../keyboard/screen-keyboard-preview";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import type { MobileKeyboardFeedback } from "./mobile-keyboard-feedback-section";
import type { SurfaceTheme } from "./theme-settings-section";
import type { TouchToolbarPreferences } from "./touch-keyboard-geometry-section";
import type { Preferences } from "../index";

export interface CreateScreenKeyboardActionsOptions {
  draft?: Preferences;
  openCommunity: () => void;
  setShowTouchSkinEditor: Dispatch<SetStateAction<boolean>>;
  saveMobileKeyboardFeedback: (settings: MobileKeyboardFeedback) => Promise<void>;
  mobileKeyboardFeedback?: MobileKeyboardFeedback;
  resetTouchKeyboardSettings: () => Promise<void>;
  openScreenKeyboard?: () => Promise<void>;
  openPanel: (action?: () => Promise<void>) => Promise<void>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates screen-keyboard theme, skin, geometry, and native-action callbacks. */
export function createScreenKeyboardActions({
  draft,
  openCommunity,
  setShowTouchSkinEditor,
  saveMobileKeyboardFeedback,
  mobileKeyboardFeedback,
  resetTouchKeyboardSettings,
  openScreenKeyboard,
  openPanel,
  setDraft,
}: CreateScreenKeyboardActionsOptions) {
  const update = (patch: Partial<Preferences>) => {
    if (draft) setDraft({ ...draft, ...patch });
  };

  return {
    onScreenKeyboardThemeChange: (screen_keyboard_theme: SurfaceTheme) =>
      update({ screen_keyboard_theme }),
    onSkinSelect: (touch_keyboard_skin: TouchKeyboardSkin) => update({ touch_keyboard_skin }),
    onToggleEditor: () => setShowTouchSkinEditor((value) => !value),
    onOpenCommunity: openCommunity,
    onDesignChange: (custom_touch_keyboard_skin: TouchKeyboardSkinDesign) =>
      update({ custom_touch_keyboard_skin }),
    onUseDesign: () => update({ touch_keyboard_skin: "custom" }),
    onCloseEditor: () => setShowTouchSkinEditor(false),
    onHeightAdjustmentChange: (touch_keyboard_height_adjustment: number) =>
      update({ touch_keyboard_height_adjustment }),
    onKeySpacingChange: (touch_key_spacing_tenths: number) => update({ touch_key_spacing_tenths }),
    onRowSpacingChange: (touch_row_spacing_tenths: number) => update({ touch_row_spacing_tenths }),
    onTouchVoiceShortcutChange: (touch_voice_shortcut: boolean) => update({ touch_voice_shortcut }),
    onToolbarChange: (touch_toolbar: TouchToolbarPreferences) => update({ touch_toolbar }),
    onTabletFullKeysChange: (tabletFullKeys: boolean) => {
      if (mobileKeyboardFeedback) {
        void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, tabletFullKeys });
      }
    },
    onReset: () => void resetTouchKeyboardSettings(),
    openScreenKeyboard: openScreenKeyboard ? () => void openPanel(openScreenKeyboard) : undefined,
  } as const;
}
