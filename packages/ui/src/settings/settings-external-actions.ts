export interface CreateSettingsExternalActionsOptions {
  mobile: boolean;
  canOpenExternalUrl: boolean;
  openExternalUrl: (url: string) => Promise<void>;
  issuesUrl: string;
  openSystemKeyboardSettings?: () => Promise<void>;
}

/** Builds fixed-link actions shared by the help and feedback settings pages. */
export function createSettingsExternalActions({
  mobile,
  canOpenExternalUrl,
  openExternalUrl,
  issuesUrl,
  openSystemKeyboardSettings,
}: CreateSettingsExternalActionsOptions) {
  return {
    onOpenDocumentation: canOpenExternalUrl
      ? () => void openExternalUrl("https://msime.app/docs/")
      : undefined,
    onOpenSystemKeyboardSettings:
      mobile && openSystemKeyboardSettings ? () => void openSystemKeyboardSettings() : undefined,
    onOpenIssues: () => void openExternalUrl(issuesUrl),
    onOpenTelegram: () => void openExternalUrl("https://t.me/msimegroup"),
  } as const;
}
