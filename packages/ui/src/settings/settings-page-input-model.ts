import type { SettingsInputPageProps } from "./settings-input-page";
import type { SettingsPageId } from "./mobile-navigation";

type InputPageState = Omit<SettingsInputPageProps, "disabled" | "hidden">;

export interface SettingsPageInputModelOptions extends InputPageState {
  page: SettingsPageId;
  busy: boolean;
}

/** Builds the input settings page props from the page controller's state. */
export function settingsPageInputModel({
  page,
  busy,
  ...state
}: SettingsPageInputModelOptions): SettingsInputPageProps {
  return {
    ...state,
    disabled: busy,
    hidden: page !== "input",
  };
}
