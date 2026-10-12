export const logo = new URL("../assets/msime.svg", import.meta.url).href;

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
