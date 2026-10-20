// 皮肤模型 src/skin.js 的单元测试，直接对源码运行：node --test packages/web-engine/test/skin.test.mjs
//
// 先用 tools/theme-catalog.mjs 从来源重新生成 src/theme-catalog.js，再导入 skin.js，所以测的永远是当前主题表和 Windows 外观表。配色与 `resolve()` 的一致性由 Rust 测试写出的 apps/desktop/tests/candidate/custom-theme-parity.json 钉住。
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
import { repoRoot, themeCatalogModule, writeThemeCatalog } from "../tools/theme-catalog.mjs";

writeThemeCatalog();
const { DEFAULT_SKIN, PLATFORM_PALETTE, SKINS, customCandidatePalette, drawnPackagePalette, normalizedColor, resolveSkin, skinImageUrl } = await import("../src/skin.js");
const { windowsLooks } = await import("../src/theme-catalog.js");
const catalog = JSON.parse(readFileSync(resolve(repoRoot, "packages/ui/src/theme/theme-catalog.json"), "utf8"));
const parity = JSON.parse(readFileSync(resolve(repoRoot, "apps/desktop/tests/candidate/custom-theme-parity.json"), "utf8"));

const COLOR = /^#[0-9A-F]{6}([0-9A-F]{2})?$/;
const PALETTE_KEYS = ["surface", "border", "text", "number", "secondary", "accent", "selected", "selectedText", "selectedNumber", "hover", "showSelectedBar", "preeditText", "caret", "selectedBar", "selectedSecondary"];
const camel = { selected_text: "selectedText", selected_number: "selectedNumber", show_selected_bar: "showSelectedBar" };
const WINDOWS_LOOKS = ["wechat", "wechat_scroll", "graphite", "willow_green", "autumn_osmanthus", "microsoft"];
// CSS 声明里能用来跳出值的字符和片段。
const BREAKOUT = /[;{}"'\\<>\n\r]|url\((?!")|\)(?!$)|javascript:/i;

/** 变量值不能跳出声明：颜色、带单位的数、关键字，或者只含安全字符的 `url("…")`。 */
function assertSafeVariables(variables) {
  for (const [name, value] of Object.entries(variables)) {
    assert.match(name, /^--[a-z-]+$/);
    assert.equal(typeof value, "string", name);
    if (name === "--msime-skin-font-family") {
      assert.match(value, /^"[^"'\\,;{}<>`\u0000-\u001f\u007f]+"$/);
      continue;
    }
    const inner = /^url\("(.*)"\)$/.exec(value);
    if (inner) {
      assert.doesNotMatch(inner[1], /["'\\()\s<>{}`]/, `${name}: ${value}`);
      assert.doesNotMatch(inner[1], /^javascript:/i);
    } else {
      assert.match(value, /^(#[0-9A-F]{6}([0-9A-F]{2})?|-?\d+(\.\d+)?(e-?\d+)?(px|ms)?|block|none|cover|contain|fill)$/, `${name}: ${value}`);
      assert.doesNotMatch(value, BREAKOUT);
    }
  }
}

function assertComplete(result) {
  for (const key of PALETTE_KEYS) {
    if (key === "showSelectedBar") assert.equal(typeof result.palette[key], "boolean");
    else assert.match(result.palette[key], COLOR, key);
  }
  assertSafeVariables(result.variables);
  assert.ok(Object.isFrozen(result) && Object.isFrozen(result.palette) && Object.isFrozen(result.geometry) && Object.isFrozen(result.variables));
}

test("the generated catalog module is deterministic and carries every Windows look", () => {
  assert.equal(themeCatalogModule(), themeCatalogModule());
  const generated = readFileSync(resolve(repoRoot, "packages/web-engine/src/theme-catalog.js"), "utf8");
  assert.equal(generated, themeCatalogModule());
  for (const look of WINDOWS_LOOKS) assert.match(generated, new RegExp(`"id":"${look}"`));
});

test("SKINS lists system, the catalog's built-in themes and the Windows looks", () => {
  const builtins = catalog.filter((entry) => entry.candidate !== null).map((entry) => entry.id);
  assert.deepEqual([...SKINS], ["system", ...builtins, ...WINDOWS_LOOKS]);
  assert.deepEqual(builtins, ["shuishan", "light", "paper", "night", "ink"]);
  assert.ok(Object.isFrozen(SKINS));
  assert.ok(!SKINS.includes("custom") && !SKINS.includes("fluent") && !SKINS.includes("canvas"));
  assert.equal(DEFAULT_SKIN, "shuishan");
  // index.d.ts 的 MsimeSkinId 必须与运行时的 SKINS 同步，否则 TypeScript 用户选不了运行时接受的外观。
  const typings = readFileSync(resolve(repoRoot, "packages/web-engine/src/index.d.ts"), "utf8");
  const union = /export type MsimeSkinId =([^;]+);/.exec(typings);
  assert.ok(union, "MsimeSkinId union not found");
  assert.deepEqual([...union[1].matchAll(/"([a-z_]+)"/g)].map((match) => match[1]), [...SKINS]);
});

test("every custom-theme-parity case gives exactly its expected palette", () => {
  assert.ok(parity.length > 0);
  for (const entry of parity) {
    const base = catalog.find((theme) => theme.id === entry.base);
    // 桌面和手机的自定义主题有浅色、深色两个皮肤槽位，固定明暗的皮肤落在另一种明暗时 resolve() 不画它（这类用例的 expected 为该明暗下的底）。网页引擎一次只画调用方给的一款皮肤，没有槽位，这款皮肤照旧画在它 base 的明暗下（见 `resolveSkin` 的说明），所以跳过这类用例。
    if (base?.appearance && (base.appearance === "dark") !== entry.dark && entry.package) continue;
    const mode = base?.appearance ?? (entry.dark ? "dark" : "light");
    const palette = entry.package ? drawnPackagePalette(entry.package, mode) : null;
    assert.deepEqual(customCandidatePalette(entry.base, entry.colors, palette), entry.expected, entry.name);
  }
});

test("resolveSkin draws the parity packages as resolve() does", () => {
  // 没有取色器的用例可以直接写成皮肤对象交给 resolveSkin：resolve() 给出的槽位必须原样出现，留给平台的槽位由平台默认配色补齐。
  let checked = 0;
  for (const entry of parity) {
    if (Object.keys(entry.colors).length > 0 || !entry.package) continue;
    for (const layout of ["horizontal", "vertical"]) {
      const result = resolveSkin({ base: entry.base, themes: entry.package.themes, candidate: entry.package.candidate }, { dark: entry.dark, layout });
      for (const [key, value] of Object.entries(entry.expected ?? {})) {
        if (value !== null) assert.equal(result.palette[camel[key] ?? key], value, `${entry.name}: ${key}`);
      }
      assertComplete(result);
    }
    checked += 1;
  }
  assert.ok(checked > 0);
});

test("every built-in id resolves in light and dark", () => {
  for (const id of SKINS) {
    for (const dark of [false, true]) {
      for (const layout of ["horizontal", "vertical"]) {
        const result = resolveSkin(id, { dark, layout });
        assert.equal(result.id, id);
        assert.equal(result.layout, layout);
        assert.equal(result.drawn, true);
        assertComplete(result);
        const theme = catalog.find((entry) => entry.id === id && entry.candidate);
        if (theme) {
          // 内置全局主题固定在自己的明暗下，配色就是主题表里那一份。
          assert.equal(result.dark, theme.appearance === "dark");
          for (const [key, value] of Object.entries(theme.candidate)) if (value !== null) assert.equal(result.palette[camel[key] ?? key], value);
          assert.equal(result.variables["--cand-bg"], theme.candidate.surface);
          assert.equal(result.variables["--accent-strong"], theme.candidate.accent);
          assert.equal(result.geometry.cornerRadius, null);
        } else {
          assert.equal(result.dark, dark);
        }
      }
    }
  }
  assert.equal(resolveSkin(undefined).id, "shuishan");
  assert.deepEqual(resolveSkin(undefined), resolveSkin("shuishan"));
});

test("system and the slots resolve() leaves to the platform use the platform defaults", () => {
  // 平台默认配色就是桌面设置页预览 `skin-card-preview` 声明的那套。
  const styles = readFileSync(resolve(repoRoot, "packages/ui/src/styles.css"), "utf8");
  const preview = styles.slice(styles.indexOf("@utility skin-card-preview"));
  const declared = (mode, name) => normalizedColor(new RegExp(`data-preview-theme="${mode}"\\][^}]*?${name}:\\s*([^;]+);`).exec(preview)[1]);
  const slots = { surface: "--cand-bg", border: "--cand-border", text: "--cand-text", number: "--cand-num", selected: "--cand-selected", hover: "--cand-hover" };
  for (const mode of ["light", "dark"]) {
    for (const [slot, name] of Object.entries(slots)) assert.equal(PLATFORM_PALETTE[mode][slot], declared(mode, name), `${mode} ${slot}`);
    const system = resolveSkin("system", { dark: mode === "dark" });
    assert.equal(system.dark, mode === "dark");
    for (const [slot, value] of Object.entries(PLATFORM_PALETTE[mode])) assert.equal(system.palette[slot], value, `${mode} ${slot}`);
    // 平台的选中底色是灰色，高亮候选画普通的文字和序号色，同候选条样式表的 var(--cand-text)、var(--cand-num)。
    assert.equal(system.palette.selectedText, system.palette.text);
    assert.equal(system.palette.selectedNumber, system.palette.number);
  }
  assert.notDeepEqual(resolveSkin("system").palette, resolveSkin("light").palette);
  // 只写了一个槽位的深色包：其余槽位是深色的平台默认值，不是另一个主题的配色。
  const surfaceOnly = resolveSkin({ candidate: { dark: { surface: "#000000" } } }, { dark: true });
  assert.deepEqual(surfaceOnly.palette, { ...resolveSkin("system", { dark: true }).palette, surface: "#000000" });
});

test("a skin object over system draws its highlighted text in its own text colour, as resolve() does", () => {
  const purple = resolveSkin({ base: "system", themes: ["light"], candidate: { light: { text: "#7B1FA2", selected: "#F3E5F5" } } });
  assert.equal(purple.palette.selected, "#F3E5F5");
  assert.equal(purple.palette.selectedText, "#7B1FA2");
  assert.equal(purple.palette.selectedNumber, purple.palette.number);
});

test("Windows looks match msime-windows over system", () => {
  const dark = resolveSkin("wechat", { dark: true });
  assert.equal(dark.palette.surface, "#151515");
  assert.equal(dark.palette.selected, "#07C160");
  assert.equal(dark.palette.showSelectedBar, false);
  assert.equal(dark.variables["--msime-skin-selected-bar"], "none");
  assert.equal(dark.geometry.cornerRadius, 5);
  assert.equal(dark.variables["--msime-skin-radius"], "5px");
  assert.equal(resolveSkin("wechat").palette.surface, "#F7F7F7");
  // 高亮候选的文字和序号色取 Windows 版样式表的 `.first .text, .first .num`：微信绿和杨柳青是绿底白字，石墨没有选中底色，只靠字色标出高亮。
  for (const dark of [false, true]) {
    for (const id of ["wechat", "willow_green"]) {
      const look = resolveSkin(id, { dark });
      assert.equal(look.palette.selectedText, "#FFFFFF", `${id} ${dark}`);
      assert.equal(look.palette.selectedNumber, "#FFFFFF");
      assert.equal(look.variables["--cand-selected-text"], "#FFFFFF");
    }
  }
  const graphite = resolveSkin("graphite");
  assert.equal(graphite.palette.selected, "#00000000");
  assert.equal(graphite.palette.selectedText, "#111827");
  assert.equal(graphite.palette.selectedNumber, "#111827");
  assert.equal(resolveSkin("graphite", { dark: true }).palette.selectedText, "#F1F3F5");
  assert.equal(resolveSkin("graphite", { dark: true }).palette.selectedNumber, "#F1F3F5");
  // 外观没给颜色的画普通的文字和序号色。
  const microsoft = resolveSkin("microsoft", { dark: true });
  assert.equal(microsoft.palette.selectedText, microsoft.palette.text);
  assert.equal(microsoft.palette.selectedNumber, microsoft.palette.number);
  // 皮肤包换了选中底色时外观的字色不再适用，画普通文字色；只改别的槽位时仍用外观的。
  const repainted = resolveSkin({ base: "wechat", candidate: { light: { selected: "#FFE399" } } });
  assert.equal(repainted.palette.selectedText, repainted.palette.text);
  assert.equal(resolveSkin({ base: "wechat", candidate: { light: { selected: "not a colour", text: "#123" } } }).palette.selectedText, "#FFFFFF");
  assert.equal(resolveSkin("microsoft").palette.showSelectedBar, true);
  // 皮肤包写了的颜色优先于外观，读不懂的颜色当没写，由外观补上。
  const pkg = resolveSkin({ base: "graphite", candidate: { light: { accent: "#123", surface: "not a colour" } } });
  assert.equal(pkg.palette.accent, "#112233");
  assert.equal(pkg.palette.surface, "#FBFBFC");
  assert.equal(pkg.geometry.cornerRadius, 3);
  assert.equal(resolveSkin({ base: "graphite", cornerRadiusDip: 12 }).geometry.cornerRadius, 12);
  // fluent 就是 system。
  assert.deepEqual(resolveSkin({ base: "fluent" }).palette, resolveSkin({ base: "system" }).palette);
});

test("the Windows looks' highlighted text colour matches the vendored msime-windows stylesheets", () => {
  // 仓库里有副本的外观逐一核对 `.first .text` 的颜色：横排的 `.first .num`、竖排的 `.first .cand-no` 与它同色，所以序号也用这一个颜色。
  const skins = resolve(repoRoot, "packages/ui/src/upstream/candidate-themes/skins");
  let checked = 0;
  for (const look of windowsLooks) {
    for (const mode of ["light", "dark"]) {
      for (const layout of ["horizontal", "vertical"]) {
        const file = resolve(skins, look.id, `${layout}_${mode}.css`);
        if (!existsSync(file)) continue;
        const css = readFileSync(file, "utf8");
        const rule = /\.first \.text,\s*\.first \.(?:num|cand-no)\s*\{\s*color:\s*(#[0-9a-f]+);/i.exec(css);
        assert.ok(rule, `${look.id} ${layout}_${mode}.css has no .first .text rule`);
        assert.equal(look[mode].selectedText, normalizedColor(rule[1]), `${look.id} ${layout}_${mode}`);
        checked += 1;
      }
    }
  }
  assert.equal(checked, 12);
});

test("unknown ids, bases and layouts throw", () => {
  assert.throws(() => resolveSkin("nope"), { name: "TypeError", message: "unknown skin: nope" });
  assert.throws(() => resolveSkin("custom"), { name: "TypeError", message: "unknown skin: custom" });
  assert.throws(() => resolveSkin("fluent"), TypeError);
  assert.throws(() => resolveSkin({ base: "custom" }), TypeError);
  assert.throws(() => resolveSkin({ base: 3 }), TypeError);
  assert.throws(() => resolveSkin("light", { layout: "diagonal" }), TypeError);
  assert.throws(() => resolveSkin(null), TypeError);
  assert.throws(() => resolveSkin(42), TypeError);
  assert.throws(() => resolveSkin([]), TypeError);
});

test("supports.layouts and themes decide whether the package is drawn, as resolve() does", () => {
  const candidate = { dark: { accent: "#AA0000" }, light: { accent: "#00AA00" } };
  const base = { candidate, minWidthDip: 200, cornerRadiusDip: 9 };
  const vertical = { ...base, supports: { layouts: ["vertical"], themes: ["dark", "light"] } };
  assert.equal(resolveSkin(vertical, { layout: "vertical" }).drawn, true);
  assert.equal(resolveSkin(vertical, { layout: "vertical" }).palette.accent, "#00AA00");
  assert.equal(resolveSkin(vertical, { layout: "vertical" }).geometry.minWidth, 200);
  const off = resolveSkin(vertical, { layout: "horizontal" });
  assert.equal(off.drawn, false);
  assert.notEqual(off.palette.accent, "#00AA00");
  assert.deepEqual(off.geometry, { canvas: null, cornerRadius: null, minWidth: null, decoration: null, background: null });
  assert.equal(off.variables["--msime-skin-radius"], undefined);
  // SkinSummary 的顶层 layouts/themes 与清单的 supports 等价。
  const lightOnly = { ...base, layouts: ["horizontal"], themes: ["light"] };
  assert.equal(resolveSkin(lightOnly, { dark: true }).drawn, false);
  assert.equal(resolveSkin(lightOnly, { dark: false }).palette.accent, "#00AA00");
  // base 为内置主题时固定画在它的明暗下：night 是深色，所以只声明浅色的包在哪种模式下都不画。
  const overNight = { ...lightOnly, base: "night" };
  for (const dark of [false, true]) {
    const result = resolveSkin(overNight, { dark });
    assert.equal(result.drawn, false);
    assert.equal(result.dark, true);
    assert.equal(result.palette.accent, catalog.find((entry) => entry.id === "night").candidate.accent);
  }
  const darkOverNight = resolveSkin({ ...overNight, themes: ["dark"] }, { dark: false });
  assert.equal(darkOverNight.drawn, true);
  assert.equal(darkOverNight.palette.accent, "#AA0000");
  assert.equal(darkOverNight.palette.selected, "#AA000024");
  // 声明写成别的东西时什么都不支持。
  assert.equal(resolveSkin({ ...base, layouts: "horizontal" }).drawn, false);
});

test("geometry follows dimension, decorationImage and the background rules", () => {
  const result = resolveSkin({
    minWidthDip: 320,
    cornerRadiusDip: 16,
    decorationTopDip: 40,
    decorationWidthDip: 120,
    decorationImage: "https://example.com/mascot.png",
    decorationAlign: "center",
    background: { image: "skins/bg.webp", fit: "stretch", opacity: 0.5 },
  });
  assert.deepEqual(result.geometry, {
    canvas: null,
    cornerRadius: 16,
    minWidth: 320,
    decoration: { url: "https://example.com/mascot.png", top: 40, width: 120, align: "center" },
    background: { url: "skins/bg.webp", fit: "stretch", opacity: 0.5 },
  });
  assert.equal(result.variables["--msime-skin-decoration-image"], 'url("https://example.com/mascot.png")');
  assert.equal(result.variables["--msime-skin-background-fit"], "fill");
  assert.equal(result.variables["--msime-skin-decoration-top"], "40px");
  // 没有 decorationImage 时用 preview；清单不写透明度时是 1。
  const legacy = resolveSkin({ decorationTopDip: 10, decorationWidthDip: 10, preview: "data:image/png;base64,AAAA", background: { image: "blob:https://a.test/1" } });
  assert.equal(legacy.geometry.decoration.url, "data:image/png;base64,AAAA");
  assert.equal(legacy.geometry.decoration.align, "right");
  assert.deepEqual(legacy.geometry.background, { url: "blob:https://a.test/1", fit: "cover", opacity: 1 });
  // 装饰带两个尺寸缺一不画。
  assert.equal(resolveSkin({ decorationTopDip: 10, decorationImage: "a.png" }).geometry.decoration, null);
});

test("schema v2 resolves a bounded motion contract and safe CSS variables", () => {
  const result = resolveSkin({
    schemaVersion: 2,
    base: "system",
    animation: { enter: "spring", exit: "fade", selection: "glow", durationMs: 220, staggerMs: 16 },
  });
  assert.deepEqual(result.animation, { enter: "spring", exit: "fade", selection: "glow", durationMs: 220, staggerMs: 16 });
  assert.equal(result.variables["--msime-skin-animation-duration"], "220ms");
  assert.equal(result.variables["--msime-skin-animation-stagger"], "16ms");
  assertSafeVariables(result.variables);
  const defaults = resolveSkin({ schemaVersion: 2 });
  assert.equal(defaults.animation.enter, "fade_scale");
  assert.equal(defaults.animation.selection, "spring");
  const legacy = resolveSkin({ schemaVersion: 1, animation: { enter: "spring" } });
  assert.equal(legacy.animation.enter, "none");
  assert.equal(resolveSkin({ schemaVersion: 2, animation: { durationMs: 9999 } }).animation.durationMs, 180);
  // v2 动画对任何 base 都生效；canvas 只额外表示九宫格与自由挂件。
  assert.equal(resolveSkin({ schemaVersion: 2, base: "system", animation: { enter: "spring" } }).animation.enter, "spring");
  assert.equal(resolveSkin({ schemaVersion: 2, base: "night", animation: { enter: "slide_up" } }).animation.enter, "slide_up");
  assert.equal(resolveSkin({ schemaVersion: 2, base: "canvas", animation: { enter: "slide_up" } }).animation.enter, "slide_up");
  assert.equal(resolveSkin({ schemaVersion: 2, canvas: true }).animation.enter, "fade_scale");
  assert.equal(resolveSkin({ schemaVersion: 2, base: "system", animation: { enter: "none", selection: "none" } }).animation.selection, "none");
});

test("base canvas requires schema v2 and the canvas flag never replaces a theme base", () => {
  assert.throws(() => resolveSkin({ base: "canvas" }), { name: "TypeError", message: "skin base canvas requires schemaVersion 2" });
  assert.throws(() => resolveSkin({ schemaVersion: 1, base: "canvas" }), { name: "TypeError", message: "skin base canvas requires schemaVersion 2" });
  assert.deepEqual(resolveSkin({ schemaVersion: 2, base: "canvas" }).palette, resolveSkin({ schemaVersion: 2, base: "system" }).palette);
  assert.deepEqual(resolveSkin({ schema_version: 2, base: "canvas" }).palette, resolveSkin({ schemaVersion: 2 }).palette);
  assert.deepEqual(resolveSkin({ base: "night", canvas: true }).palette, resolveSkin({ base: "night" }).palette);
});

test("hostile skin objects are neutralised", () => {
  const hostile = [
    "red;}body{display:none",
    "#fff;background:url(https://evil.test/x)",
    '#fff" onload="x',
    "#ffffff\\",
    "rgb(1,2,3);color:red",
    "rgba(1,2,3,2)",
    "rgb(256,0,0)",
    "expression(alert(1))",
    "url(javascript:alert(1))",
    "#12345",
    "#ggg",
    "var(--x)",
    "",
    12,
    null,
    { toString: () => "#fff" },
  ];
  for (const color of hostile) {
    const palette = { surface: color, border: color, text: color, number: color, accent: color, selected: color, hover: color, translation: color, showSelectedBar: "no" };
    const result = resolveSkin({ base: "paper", candidate: { light: palette, dark: palette } });
    const paper = catalog.find((entry) => entry.id === "paper").candidate;
    assert.equal(result.palette.surface, paper.surface, String(color));
    assert.equal(result.palette.accent, paper.accent);
    assert.equal(result.palette.showSelectedBar, true);
    assertComplete(result);
  }
  const sizes = [-1, 1e9, NaN, Infinity, "300", "1;}", null, {}];
  for (const size of sizes) {
    const result = resolveSkin({ minWidthDip: size, cornerRadiusDip: size, decorationTopDip: size, decorationWidthDip: size, decorationImage: "a.png" });
    assert.ok(result.geometry.minWidth === null || result.geometry.minWidth === 0, String(size));
    assert.ok(result.geometry.cornerRadius === null || result.geometry.cornerRadius === 0);
    assert.equal(result.geometry.decoration, null);
    assertComplete(result);
  }
  assert.equal(resolveSkin({ cornerRadiusDip: 33 }).geometry.cornerRadius, 0);
  assert.equal(resolveSkin({ minWidthDip: 1000.5 }).geometry.minWidth, 0);
  const urls = [
    "javascript:alert(1)",
    "JaVaScRiPt:alert(1)",
    "java\tscript:alert(1)",
    " javascript:alert(1)",
    "vbscript:x",
    "data:text/html,<script>alert(1)</script>",
    "data:image/svg+xml,<svg onload=alert(1)>",
    'a.png") ; } body { background: url("x',
    "a.png);}*{color:red",
    "a\\\".png",
    "url(a.png)",
    "a.png\n}",
    "file:///etc/passwd",
    "",
    42,
    { toString: () => "a.png" },
  ];
  for (const url of urls) {
    assert.equal(skinImageUrl(url), null, JSON.stringify(String(url)));
    const result = resolveSkin({ decorationTopDip: 10, decorationWidthDip: 10, decorationImage: url, background: { image: url, fit: "x;}", opacity: "1;}" } });
    assert.equal(result.geometry.decoration, null);
    assert.equal(result.geometry.background, null);
    assertComplete(result);
  }
  for (const url of ["a.png", "./skins/a.png", "/a.png", "//cdn.test/a.png", "https://a.test/a.png?x=1&y=2#z", "HTTP://A.TEST/A.PNG", "data:image/png;base64,iVBORw0KGgo=", "blob:https://a.test/uuid", "皮肤/装饰.png"]) {
    assert.equal(skinImageUrl(url), url);
  }
  // encodeURIComponent 写出的 SVG data URI 带括号和单引号：换成百分号编码后放行，图片内容不变；data URI 里的双引号、反斜杠和空白仍整条丢弃。
  const svg = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg"><rect fill="url(#g)"/></svg>')}'`;
  assert.equal(skinImageUrl(svg), svg.replaceAll("(", "%28").replaceAll(")", "%29").replaceAll("'", "%27"));
  assert.equal(decodeURIComponent(skinImageUrl(svg)), decodeURIComponent(svg));
  assertSafeVariables(resolveSkin({ background: { image: svg } }).variables);
  for (const url of ['data:image/svg+xml,a"b', "data:image/svg+xml,a\\b", "data:image/svg+xml,a b", "url(data:image/png;base64,AA)", "a(1).png"]) {
    assert.equal(skinImageUrl(url), null, url);
  }
  const clamped = resolveSkin({ background: { image: "a.png", fit: "stretch;}", opacity: 7 } });
  assert.deepEqual(clamped.geometry.background, { url: "a.png", fit: "cover", opacity: 1 });
  assert.equal(resolveSkin({ background: { image: "a.png", opacity: "abc" } }).geometry.background.opacity, 0);
  assert.equal(resolveSkin({ decorationTopDip: 1, decorationWidthDip: 1, decorationImage: "a.png", decorationAlign: "left;}" }).geometry.decoration.align, "right");
});

test("normalizedColor matches theme::normalized_color", () => {
  assert.equal(normalizedColor("#abc"), "#AABBCC");
  // `#RGBA` as `theme::normalized_color` reads it: alpha last.
  assert.equal(normalizedColor("#e8a4"), "#EE88AA44");
  assert.equal(normalizedColor(" #aabbccdd "), "#AABBCCDD");
  assert.equal(normalizedColor("transparent"), "#00000000");
  assert.equal(normalizedColor("rgb(1, 2, 3)"), "#010203");
  assert.equal(normalizedColor("rgba(255,255,255,.5)"), "#FFFFFF80");
  assert.equal(normalizedColor("RGBA(0,0,0,1e0)"), "#000000FF");
  for (const bad of ["#12345", "#1234567", "rgb(1,2)", "rgba(1,2,3)", "rgb(-1,2,3)", "rgb(1.5,2,3)", "rgba(1,2,3,-0.1)", "red", "hsl(0,0%,0%)"]) assert.equal(normalizedColor(bad), null, bad);
  for (const [alpha, expected] of [["0", "00"], ["1", "FF"], ["0.5", "80"], [".5", "80"], ["1.", "FF"], ["1e0", "FF"], ["+1", "FF"], ["-0", "00"], ["1.e0", "FF"]]) assert.equal(normalizedColor(`rgba(0,0,0,${alpha})`), `#000000${expected}`, alpha);
  for (const bad of ["1..2", ".e1", "e1", "", "."]) assert.equal(normalizedColor(`rgba(0,0,0,${bad})`), null, bad);
});

test("a long malformed alpha is rejected in linear time", () => {
  // 透明度的正则曾经在一长串数字后跟非法字符时回溯平方次：4 万位要一秒多，恶意皮肤能卡住页面。
  const hostile = `rgba(1,2,3,${"1".repeat(200000)}x)`;
  const start = performance.now();
  assert.equal(normalizedColor(hostile), null);
  assert.equal(resolveSkin({ candidate: { light: { surface: hostile, text: hostile } } }).palette.surface, PLATFORM_PALETTE.light.surface);
  assert.ok(performance.now() - start < 500, `took ${performance.now() - start} ms`);
});

test("canvas keeps panel colours separate from the readable loading fallback", () => {
  const skin = { schemaVersion: 2, base: "canvas", borderImage: { source: "dark.png", lightSource: "light.png", sliceDip: [24], widthDip: [12] },
    candidate: { light: { text: "#f5ecff", selected_text: "#fff", preedit_text: "#ffc4dc", selected_bar: "#ffd27a", accent: "#ff8fb8" } } };
  const result = resolveSkin(skin);
  assert.equal(result.geometry.canvas.borderImage.url, "light.png");
  assert.equal(result.palette.text, "#F5ECFF");
  assert.equal(result.palette.selectedText, "#FFFFFF");
  assert.equal(result.palette.preeditText, "#FFC4DC");
  assert.equal(result.palette.selectedBar, "#FFD27A");
  assert.equal(result.fallbackPalette.text, "#1A1A1A");
  assert.equal(result.fallbackPalette.surface, "#FFFFFF");
  assert.equal(result.fallbackPalette.accent, "#FF8FB8");
  assert.equal(result.fallbackPalette.secondary, "#6940A5");
  assert.equal(resolveSkin({ ...skin, borderImage: { source: "javascript:x", sliceDip: [2] } }).palette.text, "#1A1A1A");
  assertSafeVariables(result.fallbackVariables);
  assertComplete(result);
  const summary = resolveSkin({ ...skin, base: "system", canvas: true, candidate: { light: result.fallbackPalette }, panelCandidate: skin.candidate });
  assert.deepEqual(summary.palette, result.palette);
  assert.deepEqual(summary.fallbackPalette, result.fallbackPalette);
  assert.equal(resolveSkin(skin, { dark: true }).geometry.canvas.borderImage.url, "dark.png");
  assert.ok(Object.isFrozen(result.geometry.canvas.borderImage.width));
});

test("canvas geometry follows source slice, DIP widths, bounded insets and stable widget layers", () => {
  const skin = { schemaVersion: 2, base: "canvas", borderImage: { source: "panel.png", sliceDip: [20], stretchDip: [8, 16], widthDip: [4, 5, 6, 7], fill: false },
    contentInsetsDip: { left: 20 }, outerInsetsDip: { top: 30, right: 36 }, lineGapDip: 6,
    decorations: [{ image: "a.gif", widthDip: 40, heightDip: 20, zIndex: 2 }, { image: "b.apng", widthDip: 10, heightDip: 10, zIndex: -1 }, { image: "c.webp", widthDip: 32, heightDip: 32, zIndex: 2 }] };
  const geometry = resolveSkin(skin).geometry.canvas;
  assert.deepEqual(geometry.borderImage.slice, { top: 8, right: 16, bottom: 8, left: 16 });
  assert.deepEqual(geometry.borderImage.width, { top: 4, right: 5, bottom: 6, left: 7 });
  assert.equal(geometry.borderImage.fill, false);
  assert.deepEqual(geometry.contentInsets, { top: 0, right: 0, bottom: 0, left: 20 });
  assert.equal(geometry.lineGap, 6);
  assert.deepEqual(geometry.decorations.map((d) => d.url), ["b.apng", "a.gif", "c.webp"]);
  assert.equal(resolveSkin({ ...skin, base: "system" }).geometry.canvas, null);
  assert.equal(resolveSkin({ ...skin, layouts: ["vertical"] }).geometry.canvas, null);
  const hostile = resolveSkin({ ...skin, borderImage: { source: "javascript:x", sliceDip: [1] }, contentInsetsDip: { left: Infinity }, lineGapDip: "8px", decorations: [{ image: "javascript:x", widthDip: 10, heightDip: 10 }] });
  assert.equal(hostile.geometry.canvas.borderImage, null);
  assert.deepEqual(hostile.geometry.canvas.decorations, []);
  assert.equal(hostile.geometry.canvas.contentInsets.left, 0);
  assert.equal(hostile.geometry.canvas.lineGap, 0);
  assertComplete(hostile);
});


test("resolved layout preserves em, zero values and direction without changing card minimum width", () => {
  const length = (value, em = false) => ({ value, em });
  const skin = { base: "system", minWidthDip: 160, resolvedLayout: {
    cornerRadiusDip: 0, borderWidthDip: 0, itemCornerRadiusDip: 0, fitContent: false,
    horizontal: { padding: length(.3, true), itemGap: length(0), itemExtraHeight: length(4),
      itemMinWidth: length(80), numberScale: 0, numberGap: length(.2, true) },
    vertical: { padding: length(8), itemGap: length(3) },
  } };
  const horizontal = resolveSkin(skin);
  assert.equal(horizontal.geometry.minWidth, 160);
  assert.equal(horizontal.geometry.cornerRadius, 0);
  assert.equal(horizontal.variables["--msime-skin-padding"], "0.3em");
  assert.equal(horizontal.variables["--msime-skin-item-min-width"], "80px");
  assert.equal(horizontal.variables["--msime-skin-number-size"], "0em");
  assert.equal(horizontal.variables["--msime-skin-border-width"], "0px");
  assert.equal(horizontal.variables["--msime-skin-horizontal-gap"], "calc(0px + 6px)");
  assert.equal(resolveSkin({ ...skin, resolvedLayout: { ...skin.resolvedLayout, fitContent: true } }).variables["--msime-skin-horizontal-gap"], "0px");
  const vertical = resolveSkin(skin, { layout: "vertical" });
  assert.equal(vertical.variables["--msime-skin-padding"], "8px");
  assert.equal(vertical.variables["--msime-skin-vertical-gap"], "3px");
  assert.equal(resolveSkin({ ...skin, layouts: ["vertical"] }).geometry.layout, undefined);
});

test("malformed resolved geometry cannot inject CSS or unbounded sizes", () => {
  const resolved = resolveSkin({ base: "system", resolvedLayout: {
    geometry: { padding: { value: "1px;display:none", em: false }, itemGap: { value: Infinity, em: false },
      itemMinWidth: { value: -1, em: false }, numberScale: 999 },
    borderWidthDip: -1, itemCornerRadiusDip: 100000,
  } });
  assert.equal(resolved.variables["--msime-skin-padding"], undefined);
  assert.equal(resolved.variables["--msime-skin-item-min-width"], undefined);
  assert.equal(resolved.variables["--msime-skin-number-size"], undefined);
  assert.equal(resolved.variables["--msime-skin-border-width"], undefined);
  assert.equal(resolved.variables["--msime-skin-item-radius"], undefined);
});

test("resolved layout retains direction-specific switches and bounds preedit margins", () => {
  const skin = { resolvedLayout: { shadow: "soft", fitContent: true,
    horizontal: { justifyRows: false, itemCornersFollowCard: true, trimHiddenPreeditGap: true, pageArrowStyle: "triangle",
      preeditMarginDip: { top: 0, right: 4, bottom: 7, left: 2 } },
    vertical: { justifyRows: true, itemCornersFollowCard: false, trimHiddenPreeditGap: false } } };
  const horizontal = resolveSkin(skin);
  assert.equal(horizontal.geometry.layout.justifyRows, false);
  assert.equal(horizontal.geometry.layout.itemCornersFollowCard, true);
  assert.equal(horizontal.geometry.layout.pageArrowStyle, "triangle");
  assert.equal(horizontal.variables["--msime-skin-preedit-margin-bottom"], "7px");
  assert.equal(horizontal.variables["--msime-skin-preedit-margin-top"], "0px");
  assert.equal(horizontal.variables["--msime-skin-shadow-scale"], "0.5");
  assert.equal(horizontal.variables["--msime-skin-card-floor"], "0px");
  const vertical = resolveSkin(skin, { layout: "vertical" });
  assert.equal(vertical.geometry.layout.itemCornersFollowCard, false);
  assert.equal(vertical.geometry.layout.trimHiddenPreeditGap, false);
  assert.equal(vertical.variables["--msime-skin-preedit-margin-bottom"], undefined);
  const hostile = resolveSkin({ resolvedLayout: { shadow: "none;display:none", horizontal: {
    justifyRows: "true", itemCornersFollowCard: 1, pageArrowStyle: "<svg>",
    preeditMarginDip: { top: Infinity, right: "5px", bottom: -1, left: 501 } } } });
  assert.equal(hostile.geometry.layout.shadow, null);
  assert.equal(hostile.geometry.layout.justifyRows, null);
  assert.equal(hostile.geometry.layout.itemCornersFollowCard, null);
  assert.equal(hostile.geometry.layout.pageArrowStyle, null);
  assert.deepEqual(hostile.geometry.layout.preeditMargin, { top: 0, right: 0, bottom: 0, left: 0 });
  assertSafeVariables(hostile.variables);
});

test("skin font family keeps the shared single-family byte limit and declaration safety", () => {
  for (const fontFamily of ["Courier New", "示".repeat(21), "initial"]) {
    const result = resolveSkin({ fontFamily });
    assert.equal(result.geometry.fontFamily, fontFamily);
    assert.equal(result.variables["--msime-skin-font-family"], JSON.stringify(fontFamily));
    assertSafeVariables(result.variables);
  }
  for (const fontFamily of ["", "示".repeat(22), "a,b", 'a"b', "a\\b", "a;b", "a\nb", "<font>"]) {
    const result = resolveSkin({ fontFamily });
    assert.equal(result.geometry.fontFamily, undefined);
    assert.equal(result.variables["--msime-skin-font-family"], undefined);
  }
  assert.equal(resolveSkin({ fontFamily: "MSIME Fixture", layouts: ["vertical"] }).geometry.fontFamily, undefined);
});
