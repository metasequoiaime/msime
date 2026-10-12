import { expect, test } from "vitest";
import { skinFontBytes } from "../../../../packages/ui/src/skin/skin-font";
import { fontPackagePath, splitCssFontList } from "../../../../packages/ui/src/skin/toolbar-fonts";
test.each(["font/woff", "font/woff2", "font/ttf", "font/otf"])(
  "accepts bounded font bytes %s",
  (contentType) => {
    expect(new Uint8Array(skinFontBytes({ contentType, bytes: [0, 1, 255] }))).toEqual(
      new Uint8Array([0, 1, 255]),
    );
  },
);
test("rejects non-font MIME and malformed binary responses", () => {
  for (const bytes of [[], [-1], [256], [1.5], new Array(8 * 1024 * 1024 + 1).fill(0)]) {
    expect(() => skinFontBytes({ contentType: "font/woff2", bytes })).toThrow("invalid font");
  }
  expect(() => skinFontBytes({ contentType: "image/png", bytes: [0] })).toThrow("invalid font");
});
test("font sources decode package paths but never accept remote/local/traversal sources", () => {
  expect(fontPackagePath('url("fonts/\\61.woff2") format("woff2")')).toBe("fonts/a.woff2");
  for (const source of [
    'url("../a.woff2")',
    'url("\\2e\\2e/a.woff2")',
    'url("https://invalid.example/a.woff2")',
    'url("/a.woff2")',
    'local("Font")',
  ]) {
    expect(fontPackagePath(source)).toBeNull();
  }
  expect(splitCssFontList('local("a,b"), url("font.woff2") format("woff2")')).toEqual([
    'local("a,b")',
    'url("font.woff2") format("woff2")',
  ]);
  expect(splitCssFontList('local("a,(b)"), url(a\\,b.woff2), local("c")')).toEqual([
    'local("a,(b)")',
    "url(a\\,b.woff2)",
    'local("c")',
  ]);
});
