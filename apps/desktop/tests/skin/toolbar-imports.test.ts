// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { prepareToolbarImports } from "../../../../packages/ui/src/skin/toolbar-imports";

test("rebases main and recursively imported resources from their own stylesheets", async () => {
  const files: Record<string, string> = {
    "styles/parts/base.css": '@import "../colors.css"; .base{background:url("../icons/base.png")}',
    "styles/colors.css": ".color{mask-image:url(../images/mask.svg)}",
  };
  const read = vi.fn(async (relative: string) => {
    if (!(relative in files)) throw new Error("missing");
    return files[relative];
  });
  const result = await prepareToolbarImports(
    '@import "./parts/base.css"; .root{background:url("../images/root.png")}',
    "styles/toolbar.css",
    read,
  );
  expect(result.partial).toBe(false);
  expect(read.mock.calls).toEqual([["styles/parts/base.css"], ["styles/colors.css"]]);
  expect(result.css).toContain('url("images/root.png")');
  expect(result.css).toContain('url("styles/icons/base.png")');
  expect(result.css).toContain('url("images/mask.svg")');
  expect(result.css).not.toContain("@import");
});

test("preserves layer supports and media import conditions", async () => {
  const result = await prepareToolbarImports(
    '@import url("theme.css") layer(cards) supports(display: grid) screen and (min-width: 10px);',
    "toolbar.css",
    async () => ".item{display:grid}",
  );
  expect(result.partial).toBe(false);
  expect(result.css).toContain("@layer cards");
  expect(result.css).toContain("@supports (display: grid)");
  expect(result.css).toContain("@media screen and (min-width: 10px)");
  expect(result.css).toContain(".item");
});

test("keeps nested functions and commas inside import conditions", async () => {
  const result = await prepareToolbarImports(
    '@import url("theme.css") supports(selector(:is(.a, .b))) screen and (min-width: 10px);',
    "toolbar.css",
    async () => ".theme { color: red }",
  );
  expect(result.partial).toBe(false);
  expect(result.css).toContain("@supports (selector(:is(.a, .b)))");
  expect(result.css).toContain("@media screen and (min-width: 10px)");
  expect(result.css).toContain(".theme");
});

test("removes charset and decodes quoted and escaped import URLs", async () => {
  const read = vi.fn(async (relative: string) => `.${relative.replace(/\W/g, "-")} { color: red }`);
  const result = await prepareToolbarImports(
    '@charset "UTF-8"; @import \'single.css\'; @import url("escaped\\2e css");',
    "toolbar.css",
    read,
  );
  expect(result.partial).toBe(false);
  expect(read.mock.calls).toEqual([["single.css"], ["escaped.css"]]);
  expect(result.css).not.toContain("@charset");
  expect(result.css).not.toContain("@import");
});

test("treats comments as trivia between import components", async () => {
  const read = vi.fn(async () => ".imported { color: red }");
  const result = await prepareToolbarImports(
    '@import /* source */ url("parts.css") /* target */ layer /* layer */ supports(display: grid) /* supports */ screen;',
    "toolbar.css",
    read,
  );
  expect(result.partial).toBe(false);
  expect(read).toHaveBeenCalledExactlyOnceWith("parts.css");
  expect(result.css).toContain("@layer");
  expect(result.css).toContain("@supports (display: grid)");
  expect(result.css).toContain("@media screen");
  expect(result.css).toContain(".imported");
});

test("drops an import with unterminated component trivia", async () => {
  const read = vi.fn(async () => ".imported {}");
  const result = await prepareToolbarImports(
    '@import "parts.css" /* unfinished',
    "toolbar.css",
    read,
  );
  expect(result.partial).toBe(true);
  expect(read).not.toHaveBeenCalled();
  expect(result.css).not.toContain("@import");
});

test("does not activate imports that occur after ordinary rules", async () => {
  const read = vi.fn(async () => ".imported { color: red }");
  const result = await prepareToolbarImports(
    '.local { color: blue } @import "late.css";',
    "toolbar.css",
    read,
  );
  expect(result.partial).toBe(false);
  expect(read).not.toHaveBeenCalled();
  expect(result.css).toContain(".local");
  expect(result.css).not.toContain("@import");
  expect(result.css).not.toContain(".imported");
});

test("drops remote escaping cyclic and unavailable imports without losing local rules", async () => {
  const read = vi.fn(async (relative: string) => {
    if (relative === "cycle.css") return '@import "toolbar.css"; .cycle{color:red}';
    throw new Error("missing");
  });
  const result = await prepareToolbarImports(
    '@import "https://example.test/a.css"; @import "../escape.css"; @import "missing.css"; @import "cycle.css"; .local{color:blue}',
    "toolbar.css",
    read,
  );
  expect(result.partial).toBe(true);
  expect(result.css).toContain(".cycle");
  expect(result.css).toContain(".local");
  expect(result.css).not.toContain("@import");
  expect(read.mock.calls).toEqual([["missing.css"], ["cycle.css"]]);
});
