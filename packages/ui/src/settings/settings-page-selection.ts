import type { SettingsPageId } from "./settings-page-registry";

export interface CreateSettingsPageSelectionOptions {
  selectPage: (page: SettingsPageId) => void;
}

/** Adapts string page links from shared settings surfaces to typed navigation. */
export function createSettingsPageSelection({ selectPage }: CreateSettingsPageSelectionOptions) {
  return {
    onOpenPage: (page: string) => selectPage(page as SettingsPageId),
  } as const;
}
