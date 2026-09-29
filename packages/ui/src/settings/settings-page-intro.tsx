import { SettingsContentIntro, type SettingsContentIntroProps } from "./settings-content-intro";
import { mobileHeaderlessPageIds, type SettingsPageId } from "./mobile-navigation";
import { settingsPageTitle, type SettingsPageTitleItem } from "./settings-page-view-model";

export interface SettingsPageIntroProps extends Omit<
  SettingsContentIntroProps,
  "pageTitle" | "hideHeaderOnPhone"
> {
  page: SettingsPageId;
  availablePages: readonly SettingsPageTitleItem[];
}

/** Derives the page heading policy before rendering the shared settings content intro. */
export function SettingsPageIntro({
  page,
  mobile,
  availablePages,
  status,
  standalone,
}: SettingsPageIntroProps) {
  return (
    <SettingsContentIntro
      page={page}
      mobile={mobile}
      pageTitle={settingsPageTitle(availablePages, page)}
      hideHeaderOnPhone={mobileHeaderlessPageIds.includes(page)}
      status={status}
      standalone={standalone}
    />
  );
}
