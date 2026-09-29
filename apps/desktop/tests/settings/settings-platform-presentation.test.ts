import { expect, test } from "vitest";
import { settingsPlatformPresentation } from "@msime/ui";

test("derives desktop platform presentation values", () => {
  const presentation = settingsPlatformPresentation({
    android: false,
    linux: false,
    macos: true,
    harmony: false,
    ios: false,
    mobile: false,
    windows: false,
    clientHostedPlatform: false,
  });

  expect(presentation.fullwidthChord).toContain("Option");
  expect(presentation.captureBackendOptions).toContainEqual(["macos", "CoreAudio"]);
  expect(presentation.helpIntro).toContain("macOS");
  expect(presentation.issuesUrl).toContain("github.com");
});
