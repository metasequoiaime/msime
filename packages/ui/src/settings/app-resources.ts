export const logo = new URL("../assets/msime.svg", import.meta.url).href;
export const settingsIcon = new URL("../assets/settings.svg", import.meta.url).href;
export const homeIcon = new URL("../assets/home.svg", import.meta.url).href;

export const feedbackIcons = {
  github: new URL("../assets/github.svg", import.meta.url).href,
  qq: new URL("../assets/qq.svg", import.meta.url).href,
  telegram: new URL("../assets/telegram-color.svg", import.meta.url).href,
} as const;

/** Navigation icons shared by the settings registry and its host-specific shells. */
export const settingsIcons = {
  input: new URL("../assets/input.svg", import.meta.url).href,
  expression: new URL("../assets/expression.svg", import.meta.url).href,
  shortcuts: new URL("../assets/shortcut.svg", import.meta.url).href,
  dictionary: new URL("../assets/dictionary.svg", import.meta.url).href,
  skin: new URL("../assets/skin.svg", import.meta.url).href,
  appearance: new URL("../assets/appearance.svg", import.meta.url).href,
  floatingToolbar: new URL("../assets/floating-toolbar.svg", import.meta.url).href,
  screenKeyboard: new URL("../assets/screen-keyboard.svg", import.meta.url).href,
  voice: new URL("../assets/voice-input.svg", import.meta.url).href,
  handwriting: new URL("../assets/handwriting.svg", import.meta.url).href,
  utilities: new URL("../assets/utilities.svg", import.meta.url).href,
  statistics: new URL("../assets/statistics.svg", import.meta.url).href,
  plugins: new URL("../assets/plugins.svg", import.meta.url).href,
  ai: new URL("../assets/ai.svg", import.meta.url).href,
  account: new URL("../assets/account.svg", import.meta.url).href,
  community: new URL("../assets/community.svg", import.meta.url).href,
  developer: new URL("../assets/developer.svg", import.meta.url).href,
  feedback: new URL("../assets/feedback.svg", import.meta.url).href,
  about: new URL("../assets/about.svg", import.meta.url).href,
  help: new URL("../assets/help.svg", import.meta.url).href,
  vocabulary: new URL("../assets/vocabulary.svg", import.meta.url).href,
} as const;

export const windowIcons = {
  minimize: new URL("../assets/minimize.svg", import.meta.url).href,
  maximize: new URL("../assets/maximize.svg", import.meta.url).href,
  restore: new URL("../assets/restore.svg", import.meta.url).href,
  close: new URL("../assets/close.svg", import.meta.url).href,
};

export const fallbackAppVersion = "0.1.0";
const sharedReleasesPageUrl = "https://github.com/metasequoiaime/msime/releases";
export const releasesPageUrl = sharedReleasesPageUrl;
export const linuxReleasesPageUrl = sharedReleasesPageUrl;
const sharedLicenseUrl = "https://github.com/metasequoiaime/msime/blob/develop/LICENSE";
const sharedPrivacyUrl = "https://msime.app/privacy/";
export const licenseUrl = sharedLicenseUrl;
export const privacyUrl = sharedPrivacyUrl;
export const androidPrivacyUrl = sharedPrivacyUrl;
export const linuxLicenseUrl = sharedLicenseUrl;
export const linuxIssuesUrl = "https://github.com/metasequoiaime/msime/issues";
export const desktopDownloadUrl = "https://msime.app/download/";
/** 国内镜像（Cloudflare R2，见 msime-web README「国内镜像」）：`<前缀><GitHub 原地址>`，没缓存过的发布资产由镜像从 GitHub 取一次后留存。 */
export const downloadMirrorPrefix = "https://dl.msime.app/gh/";
export const documentationUrl = "https://msime.app/docs/";
export const handwritingSdkPrivacyUrl = "https://developers.google.com/ml-kit/terms";
