import { describe, expect, test } from "vitest";
import { communityLicenseLine } from "../../../../packages/ui/src/community/community-helpers";

describe("communityLicenseLine", () => {
  test("formats populated license fields in the shared display order", () => {
    expect(
      communityLicenseLine({
        assets: " CC BY 4.0 ",
        code: " MIT ",
        source: " https://example.test/source ",
      }),
    ).toBe("素材授权 CC BY 4.0 / 代码授权 MIT / 来源 https://example.test/source");
  });

  test("omits blank and missing license fields", () => {
    expect(
      communityLicenseLine({
        assets: "  ",
        code: null,
        source: "Apache-2.0",
      }),
    ).toBe("来源 Apache-2.0");
  });
});
