import { expect, test } from "vitest";
import {
  blankDraft,
  checkSkinId,
  contrastRatio,
  deriveSkinId,
  draftFromSkin,
  draftFromTheme,
  draftManifest,
  draftProblems,
  draftSchemaVersion,
  draftSkin,
  normalizeCandidateSkinDraft,
  paletteContrast,
  tomlString,
  type CandidateSkinDraft,
  type DraftImage,
} from "../../../../packages/ui/src/skin/candidate-skin-draft";
import { buildCandidateSkinPackage } from "../../../../packages/ui/src/skin/candidate-skin-package";
import {
  candidateSkinSaveMessage,
  manifestReasonMessage,
} from "../../../../packages/ui/src/skin/candidate-skin-editor-messages";
import type { ExternalSkin } from "../../../../packages/ui/src/skin/external-skins";

const image = (contentType: DraftImage["contentType"], data = "AAAA"): DraftImage => ({
  fileName: "synthetic",
  contentType,
  data,
  byteLength: 3,
  width: 40,
  height: 20,
  animated: false,
});

function named(change: Partial<CandidateSkinDraft> = {}): CandidateSkinDraft {
  return { ...blankDraft(), name: "Synthetic", id: "synthetic", ...change };
}

function lines(manifest: string): string[] {
  return manifest.split("\n");
}

test("a blank draft is a schema 1 manifest with every main colour of both modes", () => {
  const manifest = draftManifest(named(), "preview.png");
  const text = lines(manifest);
  expect(text.slice(0, 6)).toEqual([
    "schema_version = 1",
    'id = "synthetic"',
    'name = "Synthetic"',
    'version = "1.0"',
    'base = "system"',
    'preview = "preview.png"',
  ]);
  expect(text).toContain('layouts = ["horizontal", "vertical"]');
  expect(text).toContain('themes = ["light", "dark"]');
  expect(text).toContain("min_width_dip = 0");
  expect(text).toContain("corner_radius_dip = 8");
  for (const mode of ["light", "dark"]) {
    const start = text.indexOf(`[candidate.${mode}]`);
    expect(start).toBeGreaterThan(0);
    const block = text.slice(start + 1, start + 10).map((line) => line.split(" = ")[0]);
    expect(block).toEqual([
      "surface",
      "text",
      "number",
      "translation",
      "accent",
      "selected",
      "hover",
      "border",
      "show_selected_bar",
    ]);
  }
  expect(manifest).not.toContain("[animation]");
  expect(manifest).not.toContain("description");
  expect(manifest.endsWith("\n")).toBe(true);
});

test("only the modes the skin supports get a palette, and a null corner radius is left out", () => {
  const manifest = draftManifest(named({ modes: ["dark"], cornerRadius: null }), "preview.png");
  expect(manifest).toContain("[candidate.dark]");
  expect(manifest).not.toContain("[candidate.light]");
  expect(manifest).not.toContain("corner_radius_dip");
  expect(manifest).toContain('themes = ["dark"]');
});

test("animation writes schema 2 with every preset explicit, and motion alone writes them as none", () => {
  const animated = named({
    animation: {
      enabled: true,
      enter: "slide_up",
      exit: "fade",
      selection: "glow",
      durationMs: 220,
      staggerMs: 20,
    },
  });
  expect(draftSchemaVersion(animated)).toBe(2);
  const manifest = lines(draftManifest(animated, "preview.png"));
  expect(manifest[0]).toBe("schema_version = 2");
  const start = manifest.indexOf("[animation]");
  expect(manifest.slice(start + 1, start + 6)).toEqual([
    'enter = "slide_up"',
    'exit = "fade"',
    'selection = "glow"',
    "duration_ms = 220",
    "stagger_ms = 20",
  ]);

  const moving = named({
    decoration: {
      image: image("image/png"),
      topInset: 30,
      width: 80,
      align: "center",
      motion: "sway",
      motionPeriodMs: 2400,
    },
  });
  expect(draftSchemaVersion(moving)).toBe(2);
  const text = lines(draftManifest(moving, "preview.png"));
  expect(text).toContain("[candidate_window.decoration]");
  expect(text).toContain('image = "decoration.png"');
  expect(text).toContain('align = "center"');
  expect(text).toContain('motion = "sway"');
  expect(text).toContain("motion_period_ms = 2400");
  const animation = text.indexOf("[animation]");
  expect(text.slice(animation + 1, animation + 4)).toEqual([
    'enter = "none"',
    'exit = "none"',
    'selection = "none"',
  ]);
  expect(text).not.toContain("duration_ms = 180");
});

test("images get stable package paths and the background its fit and opacity", () => {
  const draft = named({
    background: { image: image("image/jpeg"), fit: "contain", opacity: 0.35 },
    decoration: {
      image: image("image/webp"),
      topInset: 24,
      width: 96,
      align: "right",
      motion: "none",
      motionPeriodMs: 3000,
    },
  });
  const text = lines(draftManifest(draft, "preview.jpg"));
  expect(text).toContain('preview = "preview.jpg"');
  expect(text).toContain('image = "background.jpg"');
  expect(text).toContain('fit = "contain"');
  expect(text).toContain("opacity = 0.35");
  expect(text).toContain('image = "decoration.webp"');
  expect(text).not.toContain("motion = ");
  expect(draftSchemaVersion(draft)).toBe(1);
  const skin = draftSkin(draft);
  expect(skin.background).toEqual({ image: "background.jpg", fit: "contain", opacity: 0.35 });
  expect(skin.decorationImage).toBe("decoration.webp");
});

test("TOML strings escape quotes, backslashes and control characters", () => {
  expect(tomlString('a"b\\c\nd\te\u0001')).toBe('"a\\"b\\\\c\\nd\\te\\u0001"');
  const manifest = draftManifest(
    named({ name: '引号"测试', description: "第一行\n第二行" }),
    "p.png",
  );
  expect(manifest).toContain('name = "引号\\"测试"');
  expect(manifest).toContain('description = "第一行\\n第二行"');
});

test("the id is derived from the name, falls back for a Chinese name and avoids installed ids", () => {
  expect(deriveSkinId("Autumn Leaves 2", new Set())).toBe("autumn-leaves-2");
  expect(deriveSkinId("秋日暖阳", new Set())).toBe("my-skin");
  expect(deriveSkinId("秋日暖阳", new Set(["my-skin", "my-skin-2"]))).toBe("my-skin-3");
  expect(deriveSkinId("Night", new Set())).toBe("night-skin");
  expect(deriveSkinId("Ünïcode", new Set())).toBe("unicode");
});

test("an id is checked as the catalog checks folder names, and an installed one asks to replace", () => {
  const installed = new Map([["sample", "Synthetic external"]]);
  expect(checkSkinId("", installed, null).kind).toBe("error");
  expect(checkSkinId("Upper", installed, null).kind).toBe("error");
  expect(checkSkinId("-dash", installed, null).kind).toBe("error");
  expect(checkSkinId("a".repeat(65), installed, null).kind).toBe("error");
  expect(checkSkinId("wechat", installed, null)).toMatchObject({ kind: "error" });
  expect(checkSkinId("default", installed, null)).toMatchObject({ kind: "error" });
  expect(checkSkinId("my.skin_1-a", installed, null)).toEqual({ kind: "ok" });
  expect(checkSkinId("sample", installed, null)).toMatchObject({
    kind: "replaces",
    name: "Synthetic external",
  });
  // 这次编辑已经保存过的皮肤再保存只是更新它。
  expect(checkSkinId("sample", installed, "sample")).toEqual({ kind: "ok" });
});

test("contrast follows WCAG and composites translucent colours first", () => {
  expect(contrastRatio("#000000", "#FFFFFF")).toBeCloseTo(21, 5);
  expect(contrastRatio("#FFFFFF", "#FFFFFF")).toBeCloseTo(1, 5);
  // 完全透明的文字就是底色本身。
  expect(contrastRatio("#00000000", "#FFFFFF")).toBeCloseTo(1, 5);
  expect(contrastRatio("#00000080", "#FFFFFF")).toBeGreaterThan(3);
  expect(contrastRatio("#00000080", "#FFFFFF")).toBeLessThan(5);

  const readings = paletteContrast(blankDraft().palettes.light, "light");
  expect(readings.map((reading) => reading.label)).toEqual([
    "候选文字 / 窗口底色",
    "候选文字 / 选中背景",
    "候选编号 / 窗口底色",
    "候选编号 / 选中背景",
    "释义文字 / 窗口底色",
    "释义文字 / 选中背景",
  ]);
  expect(readings.every((reading) => reading.ok)).toBe(true);
  const low = paletteContrast({ ...blankDraft().palettes.light, text: "#DDDDDD" }, "light");
  expect(low.filter((reading) => !reading.ok).map((reading) => reading.label)).toEqual([
    "候选文字 / 窗口底色",
    "候选文字 / 选中背景",
  ]);
});

test("a built-in theme start uses its palette and is fixed to its own mode", () => {
  const draft = draftFromTheme("night");
  expect(draft.base).toBe("night");
  expect(draft.modes).toEqual(["dark"]);
  expect(draft.palettes.dark.surface).toBe("#16262F");
  expect(draft.palettes.dark.translation).toBe("#86A6B0");
  const manifest = draftManifest({ ...draft, name: "N", id: "n" }, "preview.png");
  expect(manifest).toContain('base = "night"');
  expect(manifest).not.toContain("[candidate.light]");
});

const scanned = {
  id: "sample",
  schemaVersion: 2,
  name: "Synthetic external",
  version: "2.1",
  base: "system",
  author: "Synthetic author",
  description: "Synthetic description",
  layouts: ["vertical"],
  themes: ["dark", "light"],
  minWidthDip: 120,
  cornerRadiusDip: 12,
  decorationTopDip: 0,
  decorationWidthDip: 0,
  toolbarStylesheet: null,
  preview: "preview.png",
  candidate: {
    dark: {
      surface: "#123456",
      text: "#eeeeee",
      showSelectedBar: false,
      selected_text: "#FFFFFF",
      menu: { background: "#111111", text: "#EEEEEE", bogus: "x" },
      evil: "red;}",
    } as never,
    light: { surface: "#abcdef" },
  },
  toolbar: {
    cornerRadiusDip: 6,
    light: { background: "#FFFFFF", border: null },
    dark: {},
  },
  animation: { enter: "glow", exit: "fade", selection: "spring", durationMs: 300, staggerMs: 10 },
  license: { code: "MIT", assets: "CC0-1.0", source: null },
} satisfies ExternalSkin;

test("duplicating an installed skin keeps what the editor cannot edit and writes it back", () => {
  const draft = draftFromSkin(scanned);
  expect(draft.layouts).toEqual(["vertical"]);
  expect(draft.modes).toEqual(["light", "dark"]);
  expect(draft.palettes.dark.surface).toBe("#123456");
  expect(draft.palettes.dark.text).toBe("#EEEEEE");
  expect(draft.palettes.dark.showSelectedBar).toBe(false);
  // 包没写的颜色取空白配色。
  expect(draft.palettes.light.text).toBe(blankDraft().palettes.light.text);
  expect(draft.cornerRadius).toBe(12);
  expect(draft.minWidth).toBe(120);
  expect(draft.animation).toMatchObject({ enabled: true, enter: "glow", durationMs: 300 });
  expect(draft.carried.details.dark).toEqual({
    selected_text: "#FFFFFF",
    menu: { background: "#111111", text: "#EEEEEE" },
  });
  const manifest = draftManifest({ ...draft, id: "sample-copy" }, "preview.png");
  expect(manifest).toContain('version = "2.1"');
  expect(manifest).toContain('author = "Synthetic author"');
  expect(manifest).toContain('selected_text = "#FFFFFF"');
  expect(manifest).toContain("[candidate.dark.menu]");
  expect(manifest).not.toContain("evil");
  expect(manifest).not.toContain("bogus");
  expect(manifest).toContain("[toolbar]\ncorner_radius_dip = 6");
  expect(manifest).toContain('[toolbar.light]\nbackground = "#FFFFFF"');
  expect(manifest).not.toContain("[toolbar.dark]");
  expect(manifest).toContain('[license]\ncode = "MIT"\nassets = "CC0-1.0"');
});

test("a Windows look template keeps the look as the base", () => {
  const draft = draftFromSkin({ ...scanned, base: "system" }, {}, "wechat");
  expect(draft.base).toBe("wechat");
  expect(draftSkin(draft).base).toBe("system");
  expect(draftManifest({ ...draft, id: "w" }, "preview.png")).toContain('base = "wechat"');
});

test("any draft is normalized into the range the editor and the manifest allow", () => {
  const draft = normalizeCandidateSkinDraft({
    name: "AI",
    base: "ink",
    modes: ["light", "dark"],
    layouts: ["horizontal", "diagonal"],
    palettes: { dark: { surface: "not a colour", text: "#abc" } },
    cornerRadius: 99,
    minWidth: -5,
    decoration: { image: { data: "AAAA", contentType: "image/png" }, topInset: 0, motion: "spin" },
    background: { image: { data: "AAAA", contentType: "text/html" } },
    animation: { enabled: true, enter: "explode", durationMs: 5000 },
  });
  expect(draft.base).toBe("ink");
  expect(draft.modes).toEqual(["dark"]);
  expect(draft.layouts).toEqual(["horizontal"]);
  expect(draft.palettes.dark.surface).toBe(blankDraft().palettes.dark.surface);
  expect(draft.palettes.dark.text).toBe("#AABBCC");
  expect(draft.cornerRadius).toBe(32);
  expect(draft.minWidth).toBe(0);
  expect(draft.decoration).toMatchObject({ topInset: 1, motion: "none", align: "right" });
  expect(draft.background).toBeNull();
  expect(draft.animation).toMatchObject({ enabled: true, enter: "fade_scale", durationMs: 600 });
  expect(normalizeCandidateSkinDraft(null)).toEqual(blankDraft());
});

test("problems the page can name before saving", () => {
  const installed = new Map<string, string>();
  expect(draftProblems(named(), installed, null)).toEqual([]);
  const fields = draftProblems(
    named({ name: " ", id: "Bad", layouts: [], modes: [], description: "长".repeat(200) }),
    installed,
    null,
  ).map((problem) => problem.field);
  expect(fields).toEqual(["name", "id", "description", "layouts", "modes"]);
  const big = { ...image("image/png"), byteLength: 1_900_000, width: 4000 };
  const messages = draftProblems(
    named({ background: { image: big, fit: "cover", opacity: 1 } }),
    installed,
    null,
  ).map((problem) => problem.message);
  expect(messages.some((message) => message.includes("超过 1 MB"))).toBe(true);
  expect(messages.some((message) => message.includes("2048"))).toBe(true);
  expect(messages.some((message) => message.includes("合计太大"))).toBe(true);
});

test("the package carries the generated preview under the name its type needs", async () => {
  const draft = named({
    background: { image: image("image/jpeg", "/9j/"), fit: "cover", opacity: 1 },
  });
  const png = await buildCandidateSkinPackage(draft, async (skin, read) => {
    expect(skin.id).toBe("synthetic");
    expect((await read(skin.id, "background.jpg")).contentType).toBe("image/jpeg");
    return [0x89, 0x50, 0x4e, 0x47];
  });
  expect(png.packageId).toBe("synthetic");
  expect(Object.keys(png.files).sort()).toEqual(["background.jpg", "preview.png"]);
  expect(png.files["preview.png"]).toBe("iVBORw==");
  expect(png.files["background.jpg"]).toBe("/9j/");
  expect(png.manifest).toContain('preview = "preview.png"');

  const jpeg = await buildCandidateSkinPackage(draft, async () => [0xff, 0xd8, 0xff]);
  expect(Object.keys(jpeg.files).sort()).toEqual(["background.jpg", "preview.jpg"]);
  expect(jpeg.manifest).toContain('preview = "preview.jpg"');

  await expect(
    buildCandidateSkinPackage(draft, async () => {
      throw new Error("no canvas");
    }),
  ).rejects.toEqual({ code: "preview_failed" });
});

test("host refusals read as plain Chinese, naming the manifest problem when there is one", () => {
  expect(
    candidateSkinSaveMessage({ code: "candidate_skin_package", detail: "invalid decoration" }),
  ).toBe("皮肤没有通过校验：装饰图的设置无效：顶部留白和宽度都要大于 0，并且要选一张图片。");
  expect(candidateSkinSaveMessage({ code: "candidate_skin_package", detail: null })).toContain(
    "预览图",
  );
  expect(candidateSkinSaveMessage({ code: "candidate_skin_too_large" })).toContain("1 MB");
  expect(candidateSkinSaveMessage({ code: "storage" })).toContain("皮肤目录");
  expect(candidateSkinSaveMessage({ code: "preview_failed" })).toBe("无法生成预览图，请重试。");
  expect(candidateSkinSaveMessage(new Error("boom"))).toBe("保存失败，请重试。");
  expect(manifestReasonMessage("something new")).toBe("清单里有无效的设置（something new）。");
});
