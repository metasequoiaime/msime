import { FluentIcon } from "../core/fluent-icons";
import { ActionButton } from "../core/action-button";
import * as settings from "./settings-style";

export interface SettingsPageHeaderProps {
  title: string;
  hiddenOnPhone?: boolean;
  /** 在标题前画一个返回按钮，点击时调用它；HarmonyOS 手机在推入标签页的每个页面上都会传入。不传时页面是标签页的根页面，HarmonyOS 手机会给它的标题加内边距。 */
  onBack?: () => void;
  /** 设置 根页面，HarmonyOS 手机会让它的标题与下方的搜索胶囊拉开更大的距离。 */
  home?: boolean;
  /** Full-screen subpages use a second-level heading below the settings page title. */
  level?: 1 | 2;
  backDisabled?: boolean;
}

/** Shared page heading for the settings content surface. */
export function SettingsPageHeader({
  title,
  hiddenOnPhone,
  onBack,
  home,
  level = 1,
  backDisabled = false,
}: SettingsPageHeaderProps) {
  const classes = [
    settings.pageHeader,
    onBack ? "" : home ? settings.pageHeaderHome : settings.pageHeaderTabRoot,
    hiddenOnPhone ? "max-phone:sr-only" : "",
  ];
  return (
    <header className={classes.filter(Boolean).join(" ")}>
      {onBack && (
        <ActionButton
          action={onBack}
          className={settings.pageBackButton}
          disabled={backDisabled}
          ariaLabel="返回"
          label={<FluentIcon name="arrow_left" size={22} />}
        />
      )}
      {level === 2 ? (
        <h2 className={settings.pageTitle}>{title}</h2>
      ) : (
        <h1 className={settings.pageTitle} id="page-title">
          {title}
        </h1>
      )}
    </header>
  );
}
