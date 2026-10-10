import { expect, test } from "vitest";
import styles from "../../../../packages/ui/src/styles.css?raw";
import {
  platformCssVariables,
  platformTokens,
  settingsPlatformOf,
  settingsPlatforms,
  type PlatformTokens,
} from "../../../../packages/ui/src/theme/platform-tokens";
import { appThemeStyle, seasonAttr } from "../../../../packages/ui/src/core/app-theme-style";
import type { ResolvedAppTheme } from "../../../../packages/ui/src/core/host-contracts";

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

// Linux 设置窗口关掉了 WebKit 合成，内容区每滚一步整块重绘，分组阴影一带模糊滚动就卡（#6403）。每一层阴影的模糊半径（第三个长度）都必须是 0。
test("the Linux group shadow carries no blur", () => {
  for (const appearance of ["light", "dark"] as const) {
    const layers = platformTokens.linux[appearance].group.shadow.split(/,(?![^(]*\))/);
    expect(layers.length, appearance).toBeGreaterThan(0);
    for (const layer of layers) {
      const lengths = layer
        .replace(/rgba?\([^)]*\)/g, "")
        .trim()
        .split(/\s+/);
      expect(lengths[2] ?? "0", `${appearance}: ${layer}`).toMatch(/^0(px)?$/);
    }
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

test("only the HarmonyOS phone draws its own press fill and the inset half-pixel row hairline", () => {
  for (const platform of settingsPlatforms) {
    for (const appearance of ["light", "dark"] as const) {
      const tokens = platformTokens[platform][appearance];
      if (platform === "harmony") continue;
      expect(tokens.press, `${platform} ${appearance}`).toBe(tokens.hover);
      expect(tokens.group.dividerWidth, `${platform} ${appearance}`).toBe("1px");
      expect(tokens.group.dividerInset, `${platform} ${appearance}`).toBe("0");
      expect(tokens.group.controlGap, `${platform} ${appearance}`).toBe("12px");
    }
  }
  expect(platformTokens.harmony.light.press).toBe("rgba(0,0,0,.07)");
  expect(platformTokens.harmony.dark.press).toBe("rgba(255,255,255,.1)");
  expect(platformTokens.harmony.light.group.dividerWidth).toBe(".5px");
  expect(platformTokens.harmony.light.group.dividerInset).toBe("16px");
  expect(platformTokens.harmony.light.group.controlGap).toBe("14px");
  // 手机上 select 的值使用设计稿子页面行的 16px；2-in-1 保留其 14px 的胶囊。
  expect(platformTokens.harmony.light.select.size).toBe("16px");
  expect(platformTokens.hm2.light.select.size).toBe("14px");
  // 2-in-1 的深色按钮采用强调色，而不是原型遗留的蓝色。
  expect(platformTokens.hm2.dark.button.fg).toBe(platformTokens.hm2.dark.accent);
});

const autumnDark: ResolvedAppTheme = {
  id: "siji",
  season: "autumn",
  accent: "#F0975F",
  accent_soft: "#F0975F40",
  on_accent: "#3C2618",
  background: "#21150F",
  card: "#2E1E15",
  hair: "#2A2F2A",
};

const autumnLight: ResolvedAppTheme = {
  id: "qiushan",
  season: "autumn",
  accent: "#B5562B",
  accent_soft: "#B5562B22",
  on_accent: "#FFFFFF",
  background: "#F6E9DC",
  card: "#FFFBF6",
  hair: "#B5562B33",
};

test("an app theme recolours the HarmonyOS phone's accent, page, cards and light hairlines", () => {
  const light = appThemeStyle(autumnLight, false, "harmony") as Record<string, string>;
  expect(light).toMatchObject({
    "--accent-color": "#B5562B",
    "--accent-strong": "#B5562B",
    "--accent-soft": "#B5562B22",
    "--p-accent-text": "#B5562B",
    "--p-on-accent": "#FFFFFF",
    "--p-sw-on": "#B5562B",
    "--p-btn-fg": "#B5562B",
    "--p-seg-on-fg": "#B5562B",
    "--p-bg": "#F6E9DC",
    "--p-chrome": "#F6E9DC",
    "--p-group-bg": "#FFFBF6",
    "--p-hair": "#B5562B33",
    "--p-row-divider": "#B5562B33",
  });
  const dark = appThemeStyle(autumnDark, true, "harmony") as Record<string, string>;
  expect(dark["--p-on-accent"]).toBe("#3C2618");
  expect(dark["--p-group-bg"]).toBe("#2E1E15");
  // 深色细线沿用令牌表的值。
  expect(dark).not.toHaveProperty("--p-hair");
  expect(dark).not.toHaveProperty("--p-row-divider");
  // 它设置的每个名称都是平台层已定义的，所以是覆盖而不是新增。
  const defined = Object.keys(platformCssVariables(platformTokens.harmony.dark));
  expect(Object.keys(dark).filter((name) => !defined.includes(name))).toEqual([]);
});

test("the 2-in-1 takes only the theme's accent and keeps its own surfaces", () => {
  const style = appThemeStyle(autumnLight, false, "hm2") as Record<string, string>;
  expect(style["--accent-color"]).toBe("#B5562B");
  expect(style["--p-on-accent"]).toBe("#FFFFFF");
  for (const name of ["--p-bg", "--p-chrome", "--p-group-bg", "--p-hair", "--p-row-divider"]) {
    expect(style).not.toHaveProperty(name);
  }
});

test("without a resolved theme the platform layer is left alone", () => {
  expect(appThemeStyle(null, true, "harmony")).toEqual({});
  expect(seasonAttr(null)).toBeUndefined();
  expect(seasonAttr(autumnDark)).toBe("autumn");
});

// 手机 WebView 点按时默认盖一块蓝色或灰色高亮，原生应用里没有；页面根上把它关掉，子元素继承。
test("tapping draws no WebView highlight", () => {
  expect(styles).toMatch(/html,\s*body \{[^}]*-webkit-tap-highlight-color: transparent;/);
});

// 主按钮有自己的样式（迁移到 Tailwind 时 `primary` 只留下了引导页里的局部样式），手机上的次要按钮用平台按钮 token。
test("primary and phone secondary buttons are styled", () => {
  expect(styles).toMatch(/@utility primary \{[^}]*background: var\(--accent-strong\);/);
  expect(styles).toMatch(
    /:is\(\[data-platform="harmony"\], \[data-platform="android"\], \[data-platform="ios"\]\) & \{[^}]*background: var\(--p-btn-bg/,
  );
});

test("white text on an accent fill follows the accent's on-colour on HarmonyOS", () => {
  expect(styles).toMatch(
    /:is\(\[data-platform="harmony"\], \[data-platform="hm2"\]\)\s+:is\(\.bg-accent-strong, \.bg-accent\)\.text-white \{\s*color: var\(--p-on-accent\);/,
  );
  // 欢迎流程的品牌绿不再覆盖同一元素上的 HarmonyOS 强调色。
  expect(styles).toContain('[data-onboarding-shell][data-mobile]:not([data-platform="harmony"]) {');
});
