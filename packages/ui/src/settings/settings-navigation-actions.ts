import type { SettingsPageId } from "./settings-page-registry";

export interface CreateSettingsNavigationActionsOptions {
  selectPage: (page: SettingsPageId) => void;
  chatAvailable: boolean;
  openPanel: (action?: () => Promise<void>) => Promise<void>;
  openHandwriting?: () => Promise<void>;
  restoreDefaults: () => Promise<void>;
}

/** Creates cross-page navigation and native action callbacks used by settings panels. */
export function createSettingsNavigationActions({
  selectPage,
  chatAvailable,
  openPanel,
  openHandwriting,
  restoreDefaults,
}: CreateSettingsNavigationActionsOptions) {
  return {
    onOpenChat: chatAvailable ? () => selectPage("chat") : undefined,
    onOpenAi: () => selectPage("ai"),
    onOpenHandwriting: () => void openPanel(openHandwriting),
    onRestoreDefaults: () => void restoreDefaults(),
  } as const;
}
