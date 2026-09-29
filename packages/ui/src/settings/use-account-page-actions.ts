import { desktopDownloadUrl } from "./app-resources";
import type { AccountCommunityDestination } from "../account/account-page";

export interface UseAccountPageActionsOptions {
  hasAccountLoginReturnPage: boolean;
  finishAccountLogin: () => void;
  openLocalDesigns?: () => void;
  openCommunity?: (destination: AccountCommunityDestination) => void;
  openCloudDictionary?: () => Promise<void>;
  openCloudClipboard?: () => Promise<void>;
  mobile: boolean;
  canOpenExternalUrl: boolean;
  openExternalUrl: (url: string) => Promise<void>;
  onReplayOnboarding?: () => void;
  openAbout: () => void;
  setError: (error: string) => void;
}

/** Builds the optional navigation and cloud actions exposed by the Account page. */
export function useAccountPageActions({
  hasAccountLoginReturnPage,
  finishAccountLogin,
  openLocalDesigns,
  openCommunity,
  openCloudDictionary,
  openCloudClipboard,
  mobile,
  canOpenExternalUrl,
  openExternalUrl,
  onReplayOnboarding,
  openAbout,
  setError,
}: UseAccountPageActionsOptions) {
  const onOpenCloudDictionary = openCloudDictionary
    ? () => {
        void openCloudDictionary().catch(() => setError("无法打开云词库，请重试。"));
      }
    : undefined;
  const onOpenCloudClipboard = openCloudClipboard
    ? () => {
        void openCloudClipboard().catch(() => setError("无法打开云剪贴板，请重试。"));
      }
    : undefined;

  return {
    onCancelLogin: hasAccountLoginReturnPage ? finishAccountLogin : undefined,
    onLoginComplete: hasAccountLoginReturnPage ? finishAccountLogin : undefined,
    onOpenLocalDesigns: openLocalDesigns,
    onOpenCommunity: openCommunity,
    onOpenCloudDictionary,
    onOpenCloudClipboard,
    onOpenAbout: mobile ? openAbout : undefined,
    onOpenDesktopDownload:
      mobile && canOpenExternalUrl
        ? () => {
            void openExternalUrl(desktopDownloadUrl);
          }
        : undefined,
    onReplayOnboarding: mobile ? onReplayOnboarding : undefined,
  } as const;
}
