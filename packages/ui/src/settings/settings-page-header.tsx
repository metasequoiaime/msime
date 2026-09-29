export interface SettingsPageHeaderProps {
  title: string;
  hiddenOnPhone?: boolean;
}

/** Shared page heading for the settings content surface. */
export function SettingsPageHeader({ title, hiddenOnPhone }: SettingsPageHeaderProps) {
  return (
    <header
      className={`mb-2 flex items-center gap-2.5 pt-0 pr-6 pb-3 pl-[0.5em] ${
        hiddenOnPhone ? "max-phone:sr-only" : ""
      }`}
    >
      <h1 className="m-0 text-lg font-medium" id="page-title">
        {title}
      </h1>
    </header>
  );
}
