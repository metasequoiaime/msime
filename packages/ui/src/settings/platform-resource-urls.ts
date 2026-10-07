import {
  androidPrivacyUrl,
  licenseUrl,
  linuxIssuesUrl,
  linuxLicenseUrl,
  linuxReleasesPageUrl,
  privacyUrl,
  releasesPageUrl,
} from "./app-resources";

export interface PlatformResourceUrlsContext {
  clientHostedPlatform: boolean;
  linux: boolean;
}

export interface PlatformResourceUrls {
  releasesPageUrl: string;
  licenseUrl: string;
  issuesUrl: string;
  privacyUrl: string;
}

/** Resolves the release, legal, and support links shown by platform settings pages. */
export function platformResourceUrls({
  clientHostedPlatform,
  linux,
}: PlatformResourceUrlsContext): PlatformResourceUrls {
  return {
    releasesPageUrl: clientHostedPlatform ? linuxReleasesPageUrl : releasesPageUrl,
    licenseUrl: clientHostedPlatform ? linuxLicenseUrl : licenseUrl,
    issuesUrl: linuxIssuesUrl,
    privacyUrl: clientHostedPlatform && !linux ? androidPrivacyUrl : privacyUrl,
  };
}
