import * as settings from "./settings-style";

export interface SettingsPageHeaderProps {
  title: string;
  hiddenOnPhone?: boolean;
}

/** Shared page heading for the settings content surface. */
export function SettingsPageHeader({ title, hiddenOnPhone }: SettingsPageHeaderProps) {
  return (
    <header
      className={`${settings.pageHeader} ${hiddenOnPhone ? "max-phone:sr-only" : ""}`}
    >
      <h1 className={settings.pageTitle} id="page-title">
        {title}
      </h1>
    </header>
  );
}
