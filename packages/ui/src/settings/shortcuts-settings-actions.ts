import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";
import { defaultKeybindings } from "./keybinding-defaults";

export interface CreateShortcutsSettingsActionsOptions {
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates draft update callbacks for shortcut settings. */
export function createShortcutsSettingsActions({
  setDraft,
}: CreateShortcutsSettingsActionsOptions) {
  return {
    onKeybindingsChange: (patch: Partial<NonNullable<Preferences["keybindings"]>>) =>
      setDraft((current) =>
        current
          ? {
              ...current,
              keybindings: { ...defaultKeybindings, ...current.keybindings, ...patch },
            }
          : current,
      ),
    onInputModeHUDChange: (input_mode_hud: boolean) =>
      setDraft((current) => (current ? { ...current, input_mode_hud } : current)),
    onNumberRowSelectionChange: (number_row_selection: boolean) =>
      setDraft((current) => (current ? { ...current, number_row_selection } : current)),
  } as const;
}
