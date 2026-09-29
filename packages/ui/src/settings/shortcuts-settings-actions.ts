import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";

export interface CreateShortcutsSettingsActionsOptions {
  draft?: Preferences;
  keybindings: NonNullable<Preferences["keybindings"]>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates draft update callbacks for shortcut settings. */
export function createShortcutsSettingsActions({
  draft,
  keybindings,
  setDraft,
}: CreateShortcutsSettingsActionsOptions) {
  return {
    onKeybindingsChange: (patch: Partial<NonNullable<Preferences["keybindings"]>>) => {
      if (draft) setDraft({ ...draft, keybindings: { ...keybindings, ...patch } });
    },
    onInputModeHUDChange: (input_mode_hud: boolean) => {
      if (draft) setDraft({ ...draft, input_mode_hud });
    },
    onNumberRowSelectionChange: (number_row_selection: boolean) => {
      if (draft) setDraft({ ...draft, number_row_selection });
    },
  } as const;
}
