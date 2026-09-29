import { SettingsPageHeader } from "./settings-page-header";
import { SettingsPageStatus } from "./settings-page-status";
import {
  SettingsStandalonePages,
  type SettingsStandalonePagesProps,
} from "./settings-standalone-pages";
import type { SettingsPageId } from "./mobile-navigation";
import type { ComponentProps } from "react";

export interface SettingsContentIntroProps {
  page: SettingsPageId;
  mobile: boolean;
  pageTitle: string;
  hideHeaderOnPhone: boolean;
  status: ComponentProps<typeof SettingsPageStatus>;
  standalone: Omit<SettingsStandalonePagesProps, "page" | "mobile">;
}

/** Composes the content header, status surfaces, and non-form settings pages. */
export function SettingsContentIntro({
  page,
  mobile,
  pageTitle,
  hideHeaderOnPhone,
  status,
  standalone,
}: SettingsContentIntroProps) {
  return (
    <>
      <SettingsPageHeader title={pageTitle} hiddenOnPhone={mobile && hideHeaderOnPhone} />
      <SettingsPageStatus {...status} />
      <SettingsStandalonePages page={page} mobile={mobile} {...standalone} />
    </>
  );
}
