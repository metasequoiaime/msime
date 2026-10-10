export const logo = new URL("../assets/msime.svg", import.meta.url).href;

export const windowIcons = {
  minimize: new URL("../assets/minimize.svg", import.meta.url).href,
  maximize: new URL("../assets/maximize.svg", import.meta.url).href,
  restore: new URL("../assets/restore.svg", import.meta.url).href,
  close: new URL("../assets/close.svg", import.meta.url).href,
};

export const fallbackAppVersion = "0.1.0";
export const releasesPageUrl = "https://github.com/metasequoiaime/msime/releases";
export const linuxReleasesPageUrl = "https://github.com/metasequoiaime/msime/releases";
export const licenseUrl = "https://github.com/metasequoiaime/msime/blob/develop/LICENSE";
export const privacyUrl = "https://msime.app/privacy/";
export const androidPrivacyUrl = "https://msime.app/privacy/";
export const linuxLicenseUrl = "https://github.com/metasequoiaime/msime/blob/develop/LICENSE";
export const linuxIssuesUrl = "https://github.com/metasequoiaime/msime/issues";
export const desktopDownloadUrl = "https://msime.app/download/";
/** 国内镜像（阿里云 OSS 香港，见 msime-web README「国内镜像」）：`<前缀><GitHub 原地址>`，没缓存过的发布资产由 OSS 回源 GitHub。 */
export const downloadMirrorPrefix = "https://dl.msime.app/gh/";
export const documentationUrl = "https://msime.app/docs/";
export const handwritingSdkPrivacyUrl = "https://developers.google.com/ml-kit/terms";
