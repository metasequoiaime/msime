import type { FloatingToolbarPreferences, Preferences } from "../index";
import type {
  FloatingToolbarFontSize,
  FloatingToolbarScale,
} from "./floating-toolbar-appearance-section";
import type { FloatingToolbarComponentKey } from "./floating-toolbar-components-section";

export interface CreateFloatingToolbarActionsOptions {
  draft?: Preferences;
  preferences: FloatingToolbarPreferences;
  setDraft: (draft: Preferences) => void;
}

/** Creates draft update callbacks for the floating-toolbar settings controls. */
export function createFloatingToolbarActions({
  draft,
  preferences,
  setDraft,
}: CreateFloatingToolbarActionsOptions) {
  const update = (patch: Partial<FloatingToolbarPreferences>) => {
    if (!draft) return;
    setDraft({ ...draft, floating_toolbar: { ...preferences, ...patch } });
  };

  return {
    onEnabledChange: (enabled: boolean) => update({ enabled }),
    onScaleChange: (scale_percent: FloatingToolbarScale) => update({ scale_percent }),
    onFontSizeChange: (font_size: FloatingToolbarFontSize) => update({ font_size }),
    onComponentChange: (key: FloatingToolbarComponentKey, enabled: boolean) =>
      update({ [key]: enabled }),
  } as const;
}
