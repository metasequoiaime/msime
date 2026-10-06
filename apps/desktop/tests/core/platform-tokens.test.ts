import { expect, test } from "vitest";
import styles from "../../../../packages/ui/src/styles.css?raw";
import {
  platformCssVariables,
  platformTokens,
  settingsPlatformOf,
  settingsPlatforms,
  type PlatformTokens,
} from "../../../../packages/ui/src/theme/platform-tokens";

/** Every leaf path of a token object, so a platform that forgets a key or nests one differently shows up by name. */
function keyPaths(value: object, prefix = ""): string[] {
  return Object.entries(value).flatMap(([key, child]) =>
    child !== null && typeof child === "object"
      ? keyPaths(child as object, `${prefix}${key}.`)
      : [`${prefix}${key}`],
  );
}

function leaves(value: object): unknown[] {
  return Object.values(value).flatMap((child) =>
    child !== null && typeof child === "object" ? leaves(child as object) : [child],
  );
}

// The formatter rewrites the sheet (lowercase hex, `0.5` for `.5`, double quotes, a space after each comma), so both sides are compared in one canonical spelling.
function canonical(value: string): string {
  return value
    .toLowerCase()
    .replaceAll("'", '"')
    .replace(/\s+/g, " ")
    .replace(/\s*,\s*/g, ", ")
    .replace(/(^|[^\w.])\.(\d)/g, "$10.$2")
    .trim();
}

function declarations(selector: string): Record<string, string> {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const rule = new RegExp(`(?:^|\\n)[ \\t]*${escaped}[ \\t]*\\{([^}]*)\\}`).exec(styles);
  if (!rule) throw new Error(`no rule for ${selector} in styles.css`);
  return Object.fromEntries(
    [...rule[1].matchAll(/(--[a-z0-9-]+):\s*([^;]+);/g)].map(([, name, value]) => [
      name,
      canonical(value),
    ]),
  );
}

function canonicalVariables(tokens: PlatformTokens): Record<string, string> {
  return Object.fromEntries(
    Object.entries(platformCssVariables(tokens)).map(([name, value]) => [name, canonical(value)]),
  );
}

test("every platform and appearance carries the same complete token set", () => {
  const reference = keyPaths(platformTokens.win.light).sort();
  expect(reference.length).toBeGreaterThan(60);
  for (const platform of settingsPlatforms) {
    for (const appearance of ["light", "dark"] as const) {
      const tokens = platformTokens[platform][appearance];
      expect(keyPaths(tokens).sort(), `${platform} ${appearance}`).toEqual(reference);
      for (const leaf of leaves(tokens)) {
        if (typeof leaf === "string") expect(leaf.trim(), `${platform} ${appearance}`).not.toBe("");
        else expect(typeof leaf, `${platform} ${appearance}`).toBe("boolean");
      }
    }
  }
});

test("the stylesheet's platform layer matches the token table in both appearances", () => {
  for (const platform of settingsPlatforms) {
    const dark = declarations(`[data-platform="${platform}"]`);
    const light = {
      ...dark,
      ...declarations(`html[data-theme="light"] [data-platform="${platform}"]`),
    };
    expect(dark, `${platform} dark`).toEqual(canonicalVariables(platformTokens[platform].dark));
    expect(light, `${platform} light`).toEqual(canonicalVariables(platformTokens[platform].light));
  }
});

test("the upstream base palette carries none of the platform tokens", () => {
  const base = styles.slice(
    styles.indexOf("color-scheme: dark;"),
    styles.indexOf("}", styles.indexOf("color-scheme: light;")),
  );
  expect(base).not.toMatch(/--p-/);
  expect(base).toContain("--accent-color: #8e8cd8;");
});

test("the brand accents replace the inherited purple on every platform but the system-coloured ones", () => {
  const brand = { light: "#2C7A4B", dark: "#5FBF84" };
  for (const platform of ["mac", "hm2", "harmony", "ios", "ipad"] as const) {
    expect(platformTokens[platform].light.accent).toBe(brand.light);
    expect(platformTokens[platform].dark.accent).toBe(brand.dark);
  }
  expect(platformTokens.android.light.accent).toBe(brand.light);
  expect(platformTokens.android.dark.accent).toBe("#8FD5A6");
  expect(platformTokens.win.light.accent).toBe("#005FB8");
  expect(platformTokens.win.dark.accent).toBe("#60CDFF");
  expect(platformTokens.linux.light.accent).toBe("#3584E4");
  for (const platform of settingsPlatforms) {
    for (const appearance of ["light", "dark"] as const) {
      const accent = platformTokens[platform][appearance].accent.toLowerCase();
      expect(["#8e8cd8", "#6f6bc7", "#185c48"]).not.toContain(accent);
      // White text sits on `--accent-strong` everywhere, so it is the light-mode accent in both appearances.
      expect(platformTokens[platform][appearance].accentStrong).toBe(
        platformTokens[platform].light.accent,
      );
    }
  }
});

test("the iOS switch is system green while every other platform's follows its accent", () => {
  expect(platformTokens.ios.light.switch.on).toBe("#34C759");
  expect(platformTokens.ipad.dark.switch.on).toBe("#34C759");
  for (const platform of ["win", "mac", "linux", "hm2", "harmony", "android"] as const) {
    expect(platformTokens[platform].dark.switch.on).toBe(platformTokens[platform].dark.accent);
  }
});

test("the host and viewport choose the settings platform", () => {
  const narrow = { wide: false };
  const wide = { wide: true };
  expect(settingsPlatformOf({ platform: "windows" }, narrow)).toBe("win");
  expect(settingsPlatformOf({ platform: "macos" }, narrow)).toBe("mac");
  expect(settingsPlatformOf({ platform: "linux" }, narrow)).toBe("linux");
  expect(settingsPlatformOf({ platform: "android" }, wide)).toBe("android");
  expect(settingsPlatformOf({ platform: "ios" }, narrow)).toBe("ios");
  expect(settingsPlatformOf({ platform: "ios" }, wide)).toBe("ipad");
  expect(settingsPlatformOf({ platform: "harmony", mobile_settings: true }, wide)).toBe("harmony");
  expect(settingsPlatformOf({ platform: "harmony" }, narrow)).toBe("harmony");
  expect(settingsPlatformOf({ platform: "harmony", mobile_settings: false }, wide)).toBe("hm2");
  expect(settingsPlatformOf(undefined, narrow)).toBe("win");
});
