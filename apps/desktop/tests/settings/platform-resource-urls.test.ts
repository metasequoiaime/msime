import { expect, test } from "vitest";
import { platformResourceUrls } from "@msime/ui";

test("uses the Linux resources for a Linux host", () => {
  expect(platformResourceUrls({ clientHostedPlatform: true, linux: true })).toEqual({
    releasesPageUrl: "https://github.com/metasequoiaime/msime/releases",
    licenseUrl: "https://github.com/metasequoiaime/msime/blob/develop/LICENSE",
    issuesUrl: "https://github.com/metasequoiaime/msime/issues",
    privacyUrl: "https://github.com/metasequoiaime/msime/blob/develop/PRIVACY.md",
  });
});

test("keeps the default resources for an unhosted platform", () => {
  expect(platformResourceUrls({ clientHostedPlatform: false, linux: false })).toEqual({
    releasesPageUrl: "https://github.com/metasequoiaime/msime/releases",
    licenseUrl: "https://github.com/metasequoiaime/msime/blob/develop/LICENSE",
    issuesUrl: "https://github.com/metasequoiaime/msime/issues",
    privacyUrl: "https://msime.app/privacy/",
  });
});

test("uses the hosted mobile privacy page outside Linux", () => {
  expect(platformResourceUrls({ clientHostedPlatform: true, linux: false }).privacyUrl).toBe(
    "https://msime.app/privacy/",
  );
});
