// @vitest-environment jsdom
import { expect, test } from "vitest";
import styles from "../../../../packages/ui/src/styles.css?raw";
import * as settings from "../../../../packages/ui/src/settings/settings-style";

/*
 * The titlebar reproduces the upstream Windows one, and jsdom resolves neither custom properties nor
 * native hover, so these stay contract tests: they read the declaration the titlebar is built from
 * rather than a computed style. That source is now the utility strings rather than a stylesheet, so
 * the assertions read utilities -- the contract is the same, the place it is written moved.
 */
function utilities(value: string): string[] {
  return value.split(/\s+/).filter(Boolean);
}

/*
 * The palette lives in the stylesheet's base layer now. jsdom's CSSOM does not descend into `@layer`,
 * so the declarations are read from the source text instead -- still the declaration rather than a
 * computed style, which is the point of these tests.
 */
function tokens(source: string, selector: string): Map<string, string> {
  // Matched with its opening brace: the same selector text also appears in the `@custom-variant`
  // declaration near the top of the file, and `indexOf` would find that one first.
  const opening = `${selector} {`;
  const at = source.indexOf(opening);
  expect(at).toBeGreaterThan(-1);
  // Comments are stripped first: one of them carries a colon, and without this the declaration
  // after it is swallowed into the comment's "value".
  const body = source
    .slice(at + opening.length, source.indexOf("}", at))
    .replaceAll(/\/\*[^]*?\*\//g, "");
  return new Map(
    body
      .split(";")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        const split = line.indexOf(":");
        return [line.slice(0, split).trim(), line.slice(split + 1).trim()] as const;
      }),
  );
}

test("titlebar uses the platform tokens and each platform's caption dimensions", () => {
  const titlebar = utilities(settings.titlebar);
  expect(titlebar).toContain("bg-[var(--p-chrome)]");
  expect(titlebar).toContain("text-[var(--p-text)]");
  expect(titlebar).toContain("h-[var(--titlebar-height)]");
  // Windows 11 captions are 48 high and GNOME headerbars 46; the upstream 32 stays the fallback.
  expect(titlebar).toContain("win:[--titlebar-height:48px]");
  expect(titlebar).toContain("linux:[--titlebar-height:46px]");
  expect(titlebar).toContain("linux:border-b");

  const controls = utilities(settings.windowControls);
  expect(controls).toContain("[&>button]:h-[var(--titlebar-height)]");
  expect(controls).toContain("[&>button]:w-[46px]");
  // GNOME draws its window buttons as 24px circles.
  expect(controls).toContain("linux:[&>button]:size-6");
  expect(controls).toContain("linux:[&>button]:rounded-full");
});

test("each platform variant targets the settings root's data-platform", () => {
  for (const platform of ["win", "mac", "linux", "hm2", "harmony", "android", "ios", "ipad"])
    expect(styles).toContain(`@custom-variant ${platform} ([data-platform="${platform}"] &);`);
});

test("normal button interaction colors follow both themes", () => {
  const controls = utilities(settings.windowControls);
  expect(controls).toContain("[&>button:hover]:bg-[var(--titlebar-btn-hover)]");
  expect(controls).toContain("[&>button:active]:bg-[var(--titlebar-btn-active)]");

  // The tokens themselves still have to differ between the two palettes, or the hover would be
  // invisible in one of them.
  const dark = tokens(styles, 'html[data-theme="dark"]');
  const light = tokens(styles, 'html[data-theme="light"]');
  for (const token of [
    "--chrome-bg",
    "--text-color",
    "--titlebar-btn-hover",
    "--titlebar-btn-active",
  ]) {
    expect(dark.get(token)).toBeDefined();
    expect(light.get(token)).toBeDefined();
    expect(dark.get(token)).not.toBe(light.get(token));
  }
});

test("close interaction and keyboard focus keep dedicated styles", () => {
  const close = utilities(settings.windowClose);
  expect(close).toContain("hover:bg-[#c42b1c]!");
  expect(close).toContain("active:bg-[#a72216]!");
  // GNOME's close is a grey circle like its neighbours, not the Windows red.
  expect(close).toContain("linux:hover:bg-[var(--titlebar-btn-hover)]!");
  expect(utilities(settings.windowControls)).toContain(
    "[&>button:focus-visible]:-outline-offset-[3px]",
  );
});

test("window SVGs retain upstream sizing and light-theme contrast", () => {
  const icon = utilities(settings.windowIcon);
  expect(icon).toContain("size-[9px]");
  expect(icon).toContain("h-2.5");
  expect(icon).toContain("object-contain");
  expect(icon).toContain("[pointer-events:none]");
  // Invert first, then darken: the composed Tailwind utilities apply brightness before invert, which leaves the glyph light grey.
  expect(icon).toContain("light-theme:[filter:invert(1)_brightness(0.2)]");
  expect(utilities(settings.sidebarGlyph)).toContain(
    "light-theme:[filter:invert(1)_brightness(0.25)]",
  );

  // Close is the exception: it goes red on hover, so the white glyph must not invert there.
  const close = utilities(settings.windowClose);
  expect(close).toContain("light-theme:hover:[&_img]:[filter:none]");
  expect(close).toContain("light-theme:active:[&_img]:[filter:none]");
});
