/**
 * 「制作皮肤」编辑器的草稿：编辑器里的全部状态，以及把它变成 `skin.toml` 和图片的规则。
 *
 * 草稿是一个可以 JSON 序列化的普通对象（图片也是 base64），所以任何来源都能把一份草稿交给编辑器：空白、内置主题、Windows 外观、已安装的皮肤，以后还有 AI 生成的方案。它们都先经过 `normalizeCandidateSkinDraft` 收紧到编辑器能表示的范围。
 *
 * 这里的校验只是为了在页面上早一点说出问题；皮肤是否有效由宿主保存时走的共享安装器（client-services 的 `skin::candidate_editor`，与社区下载和 MCP 的 `create_candidate_skin` 同一条 `candidate_community::install`）决定。
 */
import { relativeLuminance } from "../core/color";
import { clamp } from "../core/number";
import { utf8ByteLength } from "../core/text";
import {
  normalizedColor,
  themeEntry,
  themeCatalog,
  type BaseGlobalTheme,
  type PackageCandidatePalette,
  type ThemeAppearance,
} from "../theme/global-theme";
import type { ExternalSkin, SkinCatalog, ToolbarPalette } from "./external-skins";

export type DraftMode = ThemeAppearance;
export type DraftLayout = "horizontal" | "vertical";
export type AnimationPreset = "none" | "fade" | "fade_scale" | "slide_up" | "spring" | "glow";
export type DecorationMotion = "none" | "float" | "sway" | "breathe" | "bounce";
export type BackgroundFit = "cover" | "contain" | "stretch";
export type DecorationAlign = "left" | "center" | "right";

/** msime-windows 的内置外观，清单 `base` 可以写它们（`catalog::WINDOWS_LOOK_IDS`）。`fluent` 就是 Windows 的原生配色，与 `system` 相同。 */
export const windowsLooks = [
  { id: "fluent", title: "Fluent" },
  { id: "wechat", title: "微信" },
  { id: "wechat_scroll", title: "微信（滚动）" },
  { id: "graphite", title: "石墨" },
  { id: "willow_green", title: "柳绿" },
  { id: "autumn_osmanthus", title: "秋桂" },
  { id: "microsoft", title: "微软" },
] as const;
export type WindowsLookId = (typeof windowsLooks)[number]["id"];
/** 清单的 `base`：`system`、内置主题或 Windows 外观。编辑器不提供 `canvas`（全贴图基底），那是以后的事。 */
export type CandidateSkinBase = BaseGlobalTheme | WindowsLookId;

export function isWindowsLook(value: string): value is WindowsLookId {
  return windowsLooks.some((look) => look.id === value);
}

/** 内置主题里能作底的那些（`GlobalTheme::is_base`）。 */
export const builtinBases = themeCatalog
  .map((entry) => entry.id)
  .filter((id): id is BaseGlobalTheme => id !== "native" && id !== "custom");

export function isCandidateSkinBase(value: unknown): value is CandidateSkinBase {
  return (
    typeof value === "string" &&
    ((builtinBases as string[]).includes(value) || isWindowsLook(value))
  );
}

/** 底色在设置页上的名字。 */
export function candidateSkinBaseTitle(base: CandidateSkinBase): string {
  const look = windowsLooks.find((entry) => entry.id === base);
  return look ? look.title : themeEntry(base).title;
}

/** 画包时用的全局主题：Windows 外观画在 `system` 上，扫描也这样报告它们（`catalog::load`）。 */
export function drawnBase(base: CandidateSkinBase): BaseGlobalTheme {
  return isWindowsLook(base) ? "system" : base;
}

/** 底色固定的明暗：内置主题里浅色、纸白是浅色，水杉、夜青、墨是深色；`system` 和 Windows 外观两种都画。皮肤只在这种明暗下使用，见 `skinSlot`。 */
export function fixedMode(base: CandidateSkinBase): DraftMode | null {
  return themeEntry(drawnBase(base)).appearance;
}

/** 编辑器里的八个主色，`[candidate.<明暗>]` 里每个宿主都画的那些。 */
export const draftColorKeys = [
  "surface",
  "text",
  "number",
  "translation",
  "accent",
  "selected",
  "hover",
  "border",
] as const;
export type DraftColorKey = (typeof draftColorKeys)[number];

export const draftColorTitles: Record<DraftColorKey, string> = {
  surface: "窗口底色",
  text: "候选文字",
  number: "候选编号",
  translation: "释义文字",
  accent: "强调色",
  selected: "选中背景",
  hover: "悬停背景",
  border: "边框",
};

/** 一种明暗的配色。颜色一律是 `#RRGGBB` 或 `#RRGGBBAA`（大写）。 */
export type DraftPalette = Record<DraftColorKey, string> & { showSelectedBar: boolean };

/** 一张选好的图片。字节以 base64 存放，草稿因此可以原样序列化。 */
export type DraftImage = {
  /** 用户选的文件名，只用来显示。 */
  fileName: string;
  contentType: "image/png" | "image/jpeg" | "image/gif" | "image/webp";
  data: string;
  byteLength: number;
  width: number;
  height: number;
  animated: boolean;
};

export type DraftBackground = { image: DraftImage; fit: BackgroundFit; opacity: number };
export type DraftDecoration = {
  image: DraftImage;
  topInset: number;
  width: number;
  align: DecorationAlign;
  motion: DecorationMotion;
  motionPeriodMs: number;
};
export type DraftAnimation = {
  /** 打开时写 schema 2 的 `[animation]`，三个预设都显式写出（macOS 只播放写出来的预设）。 */
  enabled: boolean;
  enter: AnimationPreset;
  exit: AnimationPreset;
  selection: AnimationPreset;
  durationMs: number;
  staggerMs: number;
};

/** 复制已安装皮肤时带过来、编辑器不改的部分：作者、授权、工具栏配色和细分颜色，保存时原样写回，复制出来的皮肤因此和原来的一样。 */
export type DraftCarried = {
  version: string;
  author: string | null;
  license: { code: string | null; assets: string | null; source: string | null } | null;
  toolbar: ExternalSkin["toolbar"] | null;
  /** `[candidate.<明暗>]` 里编辑器没有控件的细分颜色（`selected_text`、`menu` 等），按清单键名。 */
  details: Record<DraftMode, Record<string, string | number | Record<string, string>>>;
};

export type CandidateSkinDraft = {
  id: string;
  /** 用户自己改过 ID：改名称时不再按名称重新推导。 */
  idEdited: boolean;
  name: string;
  description: string;
  base: CandidateSkinBase;
  layouts: DraftLayout[];
  modes: DraftMode[];
  palettes: Record<DraftMode, DraftPalette>;
  /** `null` 时不写 `corner_radius_dip`，宿主用自己的圆角。 */
  cornerRadius: number | null;
  minWidth: number;
  background: DraftBackground | null;
  decoration: DraftDecoration | null;
  animation: DraftAnimation;
  carried: DraftCarried;
};

// ---- 取值范围，与 `catalog::load` 相同 ----

export const limits = {
  idBytes: 64,
  nameBytes: 80,
  descriptionBytes: 500,
  minWidth: [0, 1000],
  cornerRadius: [0, 32],
  decorationTop: [1, 500],
  decorationWidth: [1, 1000],
  motionPeriod: [1200, 8000],
  duration: [40, 600],
  stagger: [0, 80],
  /** 单张图片、全部图片和预览图的大小（`candidate_community::MAX_PACKAGE_FILE_BYTES` 等）。 */
  imageBytes: 1 << 20,
  packageBytes: 2 << 20,
  previewBytes: 256 * 1024,
  imageSide: 2048,
  packagePixels: 8_000_000,
} as const;

export const animationPresets: readonly { value: AnimationPreset; label: string }[] = [
  { value: "none", label: "无" },
  { value: "fade", label: "淡入淡出" },
  { value: "fade_scale", label: "淡入并缩放" },
  { value: "slide_up", label: "上滑" },
  { value: "spring", label: "弹性" },
  { value: "glow", label: "闪光" },
];

export const decorationMotions: readonly { value: DecorationMotion; label: string }[] = [
  { value: "none", label: "不动" },
  { value: "float", label: "上下浮动" },
  { value: "sway", label: "左右摇摆" },
  { value: "breathe", label: "呼吸" },
  { value: "bounce", label: "轻跳" },
];

/** 与 `SkinAnimation::lively()` 相同，打开动效时的起点。 */
export const livelyAnimation: DraftAnimation = {
  enabled: true,
  enter: "fade_scale",
  exit: "fade",
  selection: "spring",
  durationMs: 180,
  staggerMs: 14,
};

// ---- 起点 ----

/** 空白皮肤的配色：跟随系统的底上一套中性的浅色和深色，底色、文字和悬停取自设置页预览给 `system` 画的默认值；编号和释义比那里深一些，放在选中背景上也不低于 4.5:1。 */
export const blankPalettes: Record<DraftMode, DraftPalette> = {
  light: {
    surface: "#FFFFFF",
    text: "#1A1A1A",
    number: "#1A1A1AB3",
    translation: "#50555C",
    accent: "#005FB8",
    selected: "#005FB824",
    hover: "#1A1A1A0F",
    border: "#0000001F",
    showSelectedBar: true,
  },
  dark: {
    surface: "#202020",
    text: "#E9E8E8",
    number: "#E9E8E8B3",
    translation: "#B8B8B8",
    accent: "#60CDFF",
    selected: "#60CDFF2E",
    hover: "#FFFFFF14",
    border: "#FFFFFF1F",
    showSelectedBar: true,
  },
};

function emptyCarried(): DraftCarried {
  return {
    version: "1.0",
    author: null,
    license: null,
    toolbar: null,
    details: { light: {}, dark: {} },
  };
}

export function blankDraft(): CandidateSkinDraft {
  return {
    id: "",
    idEdited: false,
    name: "",
    description: "",
    base: "system",
    layouts: ["horizontal", "vertical"],
    modes: ["light", "dark"],
    palettes: { light: { ...blankPalettes.light }, dark: { ...blankPalettes.dark } },
    cornerRadius: 8,
    minWidth: 0,
    background: null,
    decoration: null,
    animation: { ...livelyAnimation, enabled: false },
    carried: emptyCarried(),
  };
}

/** 从内置主题开始：主题的候选配色成为它那种明暗的配色，另一种明暗用空白的；底色就是这个主题，所以只在主题自己的明暗下使用。 */
export function draftFromTheme(theme: BaseGlobalTheme): CandidateSkinDraft {
  const draft = blankDraft();
  const entry = themeEntry(theme);
  const mode = entry.appearance;
  draft.base = theme;
  if (mode) {
    draft.modes = [mode];
    const palette = entry.candidate;
    if (palette) {
      const fallback = blankPalettes[mode];
      draft.palettes[mode] = {
        surface: color(palette.surface, fallback.surface),
        text: color(palette.text, fallback.text),
        number: color(palette.number, fallback.number),
        translation: color(palette.secondary ?? palette.number, fallback.translation),
        accent: color(palette.accent, fallback.accent),
        selected: color(palette.selected, fallback.selected),
        hover: color(palette.hover, fallback.hover),
        border: color(palette.border, fallback.border),
        showSelectedBar: palette.show_selected_bar ?? fallback.showSelectedBar,
      };
    }
  }
  return draft;
}

/** 解析 Windows 外观配色时交给宿主的清单：只有底色，没有图片，宿主照装皮肤的规则补齐颜色和圆角（`check`）。 */
export function lookTemplateManifest(look: WindowsLookId): string {
  return [
    "schema_version = 1",
    'id = "template"',
    'name = "template"',
    'version = "1"',
    `base = ${tomlString(look)}`,
    "",
    "[supports]",
    'layouts = ["horizontal", "vertical"]',
    'themes = ["light", "dark"]',
    "",
    "[candidate_window]",
    "min_width_dip = 0",
    "",
  ].join("\n");
}

const safeDetailColor = /^[#(),.%\- /0-9A-Za-z]{1,80}$/;
const mainPaletteKeys = new Set<string>([
  ...draftColorKeys,
  "showSelectedBar",
  "show_selected_bar",
]);

function detailsOf(palette: Record<string, unknown> | undefined): DraftCarried["details"]["light"] {
  const details: DraftCarried["details"]["light"] = {};
  if (!palette) return details;
  for (const [key, value] of Object.entries(palette)) {
    if (mainPaletteKeys.has(key) || !/^[a-z_]{1,40}$/.test(key)) continue;
    if (typeof value === "string" && safeDetailColor.test(value)) details[key] = value;
    else if (typeof value === "number" && value >= 0 && value <= 1) details[key] = value;
    else if (key === "menu" && value && typeof value === "object") {
      const menu: Record<string, string> = {};
      for (const slot of ["background", "border", "text", "hover"]) {
        const entry = (value as Record<string, unknown>)[slot];
        if (typeof entry === "string" && safeDetailColor.test(entry)) menu[slot] = entry;
      }
      if (Object.keys(menu).length) details.menu = menu;
    }
  }
  return details;
}

/**
 * 从一款扫描到的皮肤开始：已安装的皮肤（复制），或宿主为 Windows 外观补齐的模板。
 *
 * 编辑器能表示的部分照搬；包没写的主色依次取底色主题的、再取空白的；编辑器没有控件的细分颜色、工具栏和授权放进 `carried`，保存时写回。`base` 给出时代替扫描结果的底（扫描把 Windows 外观报告成 `system`）。
 */
export function draftFromSkin(
  skin: ExternalSkin,
  images: { decoration?: DraftImage | null; background?: DraftImage | null } = {},
  base: CandidateSkinBase = skin.base,
): CandidateSkinDraft {
  const draft = blankDraft();
  draft.base = base;
  draft.name = skin.name;
  draft.description = skin.description ?? "";
  draft.layouts = (["horizontal", "vertical"] as const).filter((layout) =>
    skin.layouts.includes(layout),
  );
  const fixed = fixedMode(base);
  draft.modes = fixed
    ? [fixed]
    : (["light", "dark"] as const).filter((mode) => skin.themes.includes(mode));
  const themePalette = themeEntry(drawnBase(base)).candidate;
  for (const mode of ["light", "dark"] as const) {
    const declared = skin.candidate?.[mode] as
      | (PackageCandidatePalette & Record<string, unknown>)
      | undefined;
    const fallback = blankPalettes[mode];
    const underneath = themeEntry(drawnBase(base)).appearance === mode ? themePalette : null;
    const pick = (key: Exclude<DraftColorKey, "translation">) =>
      color(declared?.[key] ?? underneath?.[key] ?? null, fallback[key]);
    const number = pick("number");
    draft.palettes[mode] = {
      surface: pick("surface"),
      text: pick("text"),
      number,
      translation: color(declared?.translation ?? underneath?.secondary ?? null, number),
      accent: pick("accent"),
      selected: pick("selected"),
      hover: pick("hover"),
      border: pick("border"),
      showSelectedBar:
        typeof declared?.showSelectedBar === "boolean"
          ? declared.showSelectedBar
          : (underneath?.show_selected_bar ?? fallback.showSelectedBar),
    };
    draft.carried.details[mode] = detailsOf(declared);
  }
  draft.cornerRadius =
    typeof skin.cornerRadiusDip === "number" ? clamp(skin.cornerRadiusDip, 0, 32) : null;
  draft.minWidth = finite(skin.minWidthDip, 0, 0, 1000);
  if (skin.background && images.background) {
    draft.background = {
      image: images.background,
      fit: skin.background.fit,
      opacity: finite(skin.background.opacity, 1, 0, 1),
    };
  }
  if (images.decoration && skin.decorationTopDip > 0 && skin.decorationWidthDip > 0) {
    draft.decoration = {
      image: images.decoration,
      topInset: finite(skin.decorationTopDip, 24, 1, 500),
      width: finite(skin.decorationWidthDip, 96, 1, 1000),
      align: skin.decorationAlign ?? "right",
      motion: skin.decorationMotion ?? "none",
      motionPeriodMs: finite(skin.decorationMotionPeriodMs ?? 3000, 3000, 1200, 8000),
    };
  }
  if (skin.schemaVersion === 2 && skin.animation) {
    draft.animation = {
      enabled: true,
      enter: skin.animation.enter,
      exit: skin.animation.exit,
      selection: skin.animation.selection,
      durationMs: finite(skin.animation.durationMs, 180, 40, 600),
      staggerMs: finite(skin.animation.staggerMs, 14, 0, 80),
    };
  }
  draft.carried.version = skin.version || "1.0";
  draft.carried.author = skin.author;
  draft.carried.license = skin.license ?? null;
  draft.carried.toolbar = skin.toolbar ?? null;
  return draft;
}

/**
 * 把任意来源的草稿收紧到编辑器能表示的范围：缺的字段取空白草稿的，越界的数值夹到清单允许的范围，读不懂的颜色换成空白配色的。编辑器载入的每份草稿都先经过这里，所以以后交来的 AI 方案和复制来的皮肤遵守同一套规则。
 */
export function normalizeCandidateSkinDraft(input: unknown): CandidateSkinDraft {
  const blank = blankDraft();
  if (!input || typeof input !== "object") return blank;
  const value = input as Partial<Record<keyof CandidateSkinDraft, unknown>>;
  const text = (raw: unknown, fallback: string) => (typeof raw === "string" ? raw : fallback);
  const base = isCandidateSkinBase(value.base) ? value.base : blank.base;
  const fixed = fixedMode(base);
  const layouts = (["horizontal", "vertical"] as const).filter(
    (layout) => Array.isArray(value.layouts) && value.layouts.includes(layout),
  );
  const modes = fixed
    ? [fixed]
    : (["light", "dark"] as const).filter(
        (mode) => Array.isArray(value.modes) && value.modes.includes(mode),
      );
  const palettes = { ...blank.palettes };
  const rawPalettes = (value.palettes ?? {}) as Partial<Record<DraftMode, Partial<DraftPalette>>>;
  for (const mode of ["light", "dark"] as const) {
    const raw = rawPalettes[mode] ?? {};
    const fallback = blankPalettes[mode];
    const next = { ...fallback };
    for (const key of draftColorKeys) next[key] = color(raw[key] ?? null, fallback[key]);
    next.showSelectedBar =
      typeof raw.showSelectedBar === "boolean" ? raw.showSelectedBar : fallback.showSelectedBar;
    palettes[mode] = next;
  }
  const image = (raw: unknown): DraftImage | null => {
    if (!raw || typeof raw !== "object") return null;
    const candidate = raw as Partial<DraftImage>;
    if (
      typeof candidate.data !== "string" ||
      !["image/png", "image/jpeg", "image/gif", "image/webp"].includes(candidate.contentType ?? "")
    )
      return null;
    return {
      fileName: text(candidate.fileName, "image"),
      contentType: candidate.contentType as DraftImage["contentType"],
      data: candidate.data,
      byteLength: finite(
        candidate.byteLength ?? Math.floor((candidate.data.length * 3) / 4),
        0,
        0,
        Infinity,
      ),
      width: finite(candidate.width ?? 0, 0, 0, Infinity),
      height: finite(candidate.height ?? 0, 0, 0, Infinity),
      animated: candidate.animated === true,
    };
  };
  const rawBackground = value.background as Partial<DraftBackground> | null | undefined;
  const backgroundImage = image(rawBackground?.image);
  const rawDecoration = value.decoration as Partial<DraftDecoration> | null | undefined;
  const decorationImage = image(rawDecoration?.image);
  const rawAnimation = (value.animation ?? {}) as Partial<DraftAnimation>;
  const preset = (raw: unknown, fallback: AnimationPreset): AnimationPreset =>
    animationPresets.some((option) => option.value === raw) ? (raw as AnimationPreset) : fallback;
  const carried = (value.carried ?? {}) as Partial<DraftCarried>;
  return {
    id: text(value.id, ""),
    idEdited: value.idEdited === true,
    name: text(value.name, ""),
    description: text(value.description, ""),
    base,
    layouts,
    modes,
    palettes,
    cornerRadius:
      value.cornerRadius === null
        ? null
        : finite(value.cornerRadius as number, blank.cornerRadius ?? 8, 0, 32),
    minWidth: finite(value.minWidth as number, 0, 0, 1000),
    background: backgroundImage
      ? {
          image: backgroundImage,
          fit: (["cover", "contain", "stretch"] as const).includes(
            rawBackground?.fit as BackgroundFit,
          )
            ? (rawBackground?.fit as BackgroundFit)
            : "cover",
          opacity: finite(rawBackground?.opacity as number, 1, 0, 1),
        }
      : null,
    decoration: decorationImage
      ? {
          image: decorationImage,
          topInset: finite(rawDecoration?.topInset as number, 24, 1, 500),
          width: finite(rawDecoration?.width as number, 96, 1, 1000),
          align: (["left", "center", "right"] as const).includes(
            rawDecoration?.align as DecorationAlign,
          )
            ? (rawDecoration?.align as DecorationAlign)
            : "right",
          motion: decorationMotions.some((option) => option.value === rawDecoration?.motion)
            ? (rawDecoration?.motion as DecorationMotion)
            : "none",
          motionPeriodMs: Math.round(
            finite(rawDecoration?.motionPeriodMs as number, 3000, 1200, 8000),
          ),
        }
      : null,
    animation: {
      enabled: rawAnimation.enabled === true,
      enter: preset(rawAnimation.enter, livelyAnimation.enter),
      exit: preset(rawAnimation.exit, livelyAnimation.exit),
      selection: preset(rawAnimation.selection, livelyAnimation.selection),
      durationMs: Math.round(finite(rawAnimation.durationMs as number, 180, 40, 600)),
      staggerMs: Math.round(finite(rawAnimation.staggerMs as number, 14, 0, 80)),
    },
    carried: {
      version: typeof carried.version === "string" && carried.version ? carried.version : "1.0",
      author: typeof carried.author === "string" ? carried.author : null,
      license: carried.license ?? null,
      toolbar: carried.toolbar ?? null,
      details: {
        light: detailsOf(carried.details?.light as Record<string, unknown> | undefined),
        dark: detailsOf(carried.details?.dark as Record<string, unknown> | undefined),
      },
    },
  };
}

function finite(value: number, fallback: number, minimum: number, maximum: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? clamp(value, minimum, maximum)
    : fallback;
}

function color(value: string | null | undefined, fallback: string): string {
  return (typeof value === "string" && normalizedColor(value)) || fallback;
}

// ---- ID ----

/** 外部皮肤不能用的 ID：全局主题、msime-windows 的内置外观、`canvas` 和 `default`（`catalog::is_reserved`）。 */
export const reservedSkinIds: readonly string[] = [
  "system",
  "native",
  "shuishan",
  "light",
  "paper",
  "night",
  "ink",
  "custom",
  ...windowsLooks.map((look) => look.id),
  "canvas",
  "default",
];

/** 从名称推出 ID：名称里的英文字母和数字连成小写，其余换成连字符；没有可用的字符时用 `my-skin`。与已安装的重名时加 `-2`、`-3`。 */
export function deriveSkinId(name: string, taken: ReadonlySet<string>): string {
  let stem = name
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48)
    .replace(/-+$/, "");
  if (!stem || reservedSkinIds.includes(stem)) stem = stem ? `${stem}-skin` : "my-skin";
  if (!taken.has(stem)) return stem;
  for (let suffix = 2; ; suffix++) {
    const id = `${stem}-${suffix}`;
    if (!taken.has(id)) return id;
  }
}

export type IdCheck =
  | { kind: "ok" }
  | { kind: "error"; message: string }
  /** 本地已有这个 ID 的皮肤，而且不是这次编辑保存的那款：保存时要用户确认替换。 */
  | { kind: "replaces"; name: string; message: string };

/** 与 `catalog::is_external_id` 同一条规则，另外说出它会不会替换本地已有的皮肤。`own` 是这次编辑已经保存过的 ID，再保存只是更新它。 */
export function checkSkinId(
  id: string,
  installed: ReadonlyMap<string, string>,
  own: string | null,
): IdCheck {
  if (!id) return { kind: "error", message: "请填写皮肤 ID。" };
  if (utf8ByteLength(id) > limits.idBytes)
    return { kind: "error", message: "皮肤 ID 最多 64 个字符。" };
  if (!/^[a-z0-9][a-z0-9._-]*$/.test(id))
    return {
      kind: "error",
      message: "皮肤 ID 只能用小写英文字母、数字和 . _ -，并以字母或数字开头。",
    };
  if (reservedSkinIds.includes(id))
    return { kind: "error", message: `“${id}”是内置主题或外观的名字，请换一个 ID。` };
  const existing = installed.get(id);
  if (existing !== undefined && id !== own)
    return {
      kind: "replaces",
      name: existing,
      message: `本地已有 ID 为“${id}”的皮肤「${existing}」，保存时会先问你是否替换它。`,
    };
  return { kind: "ok" };
}

export function installedSkins(catalog: SkinCatalog | null): Map<string, string> {
  return new Map((catalog?.packages ?? []).map((skin) => [skin.id, skin.name]));
}

// ---- 校验 ----

export type DraftProblem = { field: string; message: string };

/** 保存前在页面上就能说出的问题。宿主的安装器仍会再查一遍，这里查不到的（例如图片解码）由它报告。 */
export function draftProblems(
  draft: CandidateSkinDraft,
  installed: ReadonlyMap<string, string>,
  own: string | null,
): DraftProblem[] {
  const problems: DraftProblem[] = [];
  const name = draft.name.trim();
  if (!name) problems.push({ field: "name", message: "请给皮肤起个名字。" });
  else if (utf8ByteLength(name) > limits.nameBytes)
    problems.push({ field: "name", message: "名称太长了：最多 80 个字节（约 26 个汉字）。" });
  const id = checkSkinId(draft.id, installed, own);
  if (id.kind === "error") problems.push({ field: "id", message: id.message });
  if (utf8ByteLength(draft.description.trim()) > limits.descriptionBytes)
    problems.push({
      field: "description",
      message: "说明太长了：最多 500 个字节（约 166 个汉字）。",
    });
  if (!draft.layouts.length)
    problems.push({ field: "layouts", message: "至少选择一种候选排列（横排或竖排）。" });
  if (!draft.modes.length)
    problems.push({ field: "modes", message: "至少选择一种明暗模式（浅色或深色）。" });
  const images = draftImages(draft);
  for (const [, image] of images) {
    if (image.byteLength > limits.imageBytes)
      problems.push({
        field: "images",
        message: `图片「${image.fileName}」超过 1 MB，请换一张小一些的。`,
      });
    if (image.width > limits.imageSide || image.height > limits.imageSide)
      problems.push({
        field: "images",
        message: `图片「${image.fileName}」的边长超过 2048 像素，请换一张小一些的。`,
      });
  }
  const total = images.reduce((sum, [, image]) => sum + image.byteLength, 0);
  // 预览图另占最多 256 KB，所以留出它的位置。
  if (total + limits.previewBytes > limits.packageBytes)
    problems.push({
      field: "images",
      message: "背景图和装饰图合计太大：连同自动生成的预览图不能超过 2 MB。",
    });
  return problems;
}

// ---- 对比度 ----

type Rgba = { r: number; g: number; b: number; a: number };

function rgba(value: string): Rgba {
  const hex = (normalizedColor(value) ?? "#000000").slice(1);
  return {
    r: parseInt(hex.slice(0, 2), 16),
    g: parseInt(hex.slice(2, 4), 16),
    b: parseInt(hex.slice(4, 6), 16),
    a: hex.length === 8 ? parseInt(hex.slice(6, 8), 16) / 255 : 1,
  };
}

/** `top` 画在不透明的 `bottom` 上之后的颜色。 */
function over(top: Rgba, bottom: Rgba): Rgba {
  const mix = (front: number, back: number) => Math.round(front * top.a + back * (1 - top.a));
  return { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a: 1 };
}

function luminance(value: Rgba): number {
  return relativeLuminance((value.r << 16) | (value.g << 8) | value.b);
}

/** 两个颜色的 WCAG 对比度。`foreground` 带透明度时先叠到 `background` 上；`background` 带透明度时叠到 `canvas` 上（默认白色）。 */
export function contrastRatio(foreground: string, background: string, canvas = "#FFFFFF"): number {
  const back = over(rgba(background), rgba(canvas));
  const front = over(rgba(foreground), back);
  const a = luminance(front);
  const b = luminance(back);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** 正文对比度的下限（WCAG AA）。 */
export const minimumContrast = 4.5;

export type ContrastReading = { label: string; ratio: number; ok: boolean };

/** 一种明暗下三种文字分别放在窗口底色和选中背景上的对比度。半透明的窗口底色按浅色模式叠在白色、深色模式叠在近黑色上量。 */
export function paletteContrast(palette: DraftPalette, mode: DraftMode): ContrastReading[] {
  const canvas = mode === "light" ? "#FFFFFF" : "#1F1F1F";
  const surface = over(rgba(palette.surface), rgba(canvas));
  const surfaceHex = hex(surface);
  const selectedHex = hex(over(rgba(palette.selected), surface));
  const readings: ContrastReading[] = [];
  for (const [key, title] of [
    ["text", "候选文字"],
    ["number", "候选编号"],
    ["translation", "释义文字"],
  ] as const) {
    for (const [background, place] of [
      [surfaceHex, "窗口底色"],
      [selectedHex, "选中背景"],
    ] as const) {
      const ratio = contrastRatio(palette[key], background);
      readings.push({
        label: `${title} / ${place}`,
        ratio,
        ok: ratio >= minimumContrast,
      });
    }
  }
  return readings;
}

function hex(value: Rgba): string {
  return `#${[value.r, value.g, value.b]
    .map((channel) => channel.toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase()}`;
}

// ---- 清单与包 ----

/** 草稿要用 schema 2：打开了动效，或装饰图有待机动效。其余写 schema 1，任何版本的宿主都认。 */
export function draftSchemaVersion(draft: CandidateSkinDraft): 1 | 2 {
  return draft.animation.enabled || (draft.decoration && draft.decoration.motion !== "none")
    ? 2
    : 1;
}

const imageExtensions: Record<DraftImage["contentType"], string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
};

/** 包里的图片和它们的路径：`background.<扩展名>`、`decoration.<扩展名>`。预览图另外生成。 */
export function draftImages(draft: CandidateSkinDraft): [string, DraftImage][] {
  const images: [string, DraftImage][] = [];
  if (draft.background)
    images.push([
      `background.${imageExtensions[draft.background.image.contentType]}`,
      draft.background.image,
    ]);
  if (draft.decoration)
    images.push([
      `decoration.${imageExtensions[draft.decoration.image.contentType]}`,
      draft.decoration.image,
    ]);
  return images;
}

/** TOML 基本字符串。 */
export function tomlString(value: string): string {
  let out = '"';
  for (const character of value) {
    const code = character.codePointAt(0) ?? 0;
    if (character === '"') out += '\\"';
    else if (character === "\\") out += "\\\\";
    else if (character === "\n") out += "\\n";
    else if (character === "\r") out += "\\r";
    else if (character === "\t") out += "\\t";
    else if (code < 0x20 || code === 0x7f) out += `\\u${code.toString(16).padStart(4, "0")}`;
    else out += character;
  }
  return `${out}"`;
}

function tomlNumber(value: number): string {
  const rounded = Math.round(value * 1000) / 1000;
  return Number.isInteger(rounded) ? String(rounded) : rounded.toString();
}

/**
 * 草稿写成的 `skin.toml`。`preview` 是随包生成的预览图的文件名。
 *
 * 只写编辑器管的键和复制时带来的部分；每种勾选的明暗都写全八个主色和选中竖条，所以宿主画出来的就是预览里的样子，不依赖各宿主自己的默认色。
 */
export function draftManifest(draft: CandidateSkinDraft, preview: string): string {
  const schema = draftSchemaVersion(draft);
  const lines: string[] = [
    `schema_version = ${schema}`,
    `id = ${tomlString(draft.id)}`,
    `name = ${tomlString(draft.name.trim())}`,
    `version = ${tomlString(draft.carried.version || "1.0")}`,
    `base = ${tomlString(draft.base)}`,
  ];
  const description = draft.description.trim();
  if (description) lines.push(`description = ${tomlString(description)}`);
  if (draft.carried.author) lines.push(`author = ${tomlString(draft.carried.author)}`);
  lines.push(`preview = ${tomlString(preview)}`);
  lines.push(
    "",
    "[supports]",
    `layouts = [${draft.layouts.map(tomlString).join(", ")}]`,
    `themes = [${draft.modes.map(tomlString).join(", ")}]`,
    "",
    "[candidate_window]",
    `min_width_dip = ${tomlNumber(draft.minWidth)}`,
  );
  if (draft.cornerRadius !== null)
    lines.push(`corner_radius_dip = ${tomlNumber(draft.cornerRadius)}`);
  const images = new Map(draftImages(draft).map(([path, image]) => [image, path]));
  if (draft.decoration) {
    const decoration = draft.decoration;
    lines.push(
      "",
      "[candidate_window.decoration]",
      `top_inset_dip = ${tomlNumber(decoration.topInset)}`,
      `width_dip = ${tomlNumber(decoration.width)}`,
      `image = ${tomlString(images.get(decoration.image) ?? "")}`,
      `align = ${tomlString(decoration.align)}`,
    );
    if (decoration.motion !== "none")
      lines.push(
        `motion = ${tomlString(decoration.motion)}`,
        `motion_period_ms = ${Math.round(decoration.motionPeriodMs)}`,
      );
  }
  if (draft.background) {
    lines.push(
      "",
      "[candidate_window.background]",
      `image = ${tomlString(images.get(draft.background.image) ?? "")}`,
      `fit = ${tomlString(draft.background.fit)}`,
      `opacity = ${tomlNumber(draft.background.opacity)}`,
    );
  }
  for (const mode of draft.modes) {
    const palette = draft.palettes[mode];
    lines.push("", `[candidate.${mode}]`);
    for (const key of draftColorKeys) lines.push(`${key} = ${tomlString(palette[key])}`);
    lines.push(`show_selected_bar = ${palette.showSelectedBar}`);
    const details = draft.carried.details[mode];
    for (const [key, value] of Object.entries(details)) {
      if (key === "menu" || typeof value === "object") continue;
      lines.push(`${key} = ${typeof value === "number" ? tomlNumber(value) : tomlString(value)}`);
    }
    const menu = details.menu;
    if (menu && typeof menu === "object") {
      lines.push("", `[candidate.${mode}.menu]`);
      for (const [key, value] of Object.entries(menu)) lines.push(`${key} = ${tomlString(value)}`);
    }
  }
  const toolbar = draft.carried.toolbar;
  if (toolbar) {
    const modes = (["light", "dark"] as const).filter((mode) =>
      Object.values(toolbar[mode] ?? {}).some((value) => typeof value === "string"),
    );
    if (typeof toolbar.cornerRadiusDip === "number" || modes.length) {
      lines.push("", "[toolbar]");
      if (typeof toolbar.cornerRadiusDip === "number")
        lines.push(`corner_radius_dip = ${tomlNumber(clamp(toolbar.cornerRadiusDip, 0, 32))}`);
      for (const mode of modes) {
        lines.push("", `[toolbar.${mode}]`);
        for (const [key, value] of Object.entries(toolbar[mode] as ToolbarPalette))
          if (typeof value === "string" && safeDetailColor.test(value))
            lines.push(`${key} = ${tomlString(value)}`);
      }
    }
  }
  if (schema === 2) {
    // schema 2 省略 `[animation]` 时 Windows 和网页按默认组合播放，所以关掉动效也要显式写成 none。
    const animation = draft.animation.enabled
      ? draft.animation
      : { ...draft.animation, enter: "none", exit: "none", selection: "none" };
    lines.push(
      "",
      "[animation]",
      `enter = ${tomlString(animation.enter)}`,
      `exit = ${tomlString(animation.exit)}`,
      `selection = ${tomlString(animation.selection)}`,
    );
    if (draft.animation.enabled)
      lines.push(
        `duration_ms = ${Math.round(animation.durationMs)}`,
        `stagger_ms = ${Math.round(animation.staggerMs)}`,
      );
  }
  const license = draft.carried.license;
  if (license && (license.code || license.assets || license.source)) {
    lines.push("", "[license]");
    if (license.code) lines.push(`code = ${tomlString(license.code)}`);
    if (license.assets) lines.push(`assets = ${tomlString(license.assets)}`);
    if (license.source) lines.push(`source = ${tomlString(license.source)}`);
  }
  return `${lines.join("\n")}\n`;
}

/** 交给宿主的包：清单，以及包内路径到标准 base64 的全部图片。 */
export type CandidateSkinPackageRequest = {
  packageId: string;
  manifest: string;
  files: Record<string, string>;
};

/** 宿主的回答：装好的 ID，以及是否替换了同名的皮肤。 */
export type CreatedCandidateSkin = { id: string; replaced: boolean };

/**
 * 宿主为「制作皮肤」提供的两件事，都走 client-services 的 `skin::candidate_editor`。
 *
 * - `check`：按皮肤目录的规则读一份清单（和图片），不写皮肤目录，回答扫描会列出的那一项。编辑器用它向宿主要 Windows 外观补齐后的配色。
 * - `create`：经共享安装器装进本机皮肤目录。`replace` 为假时同名皮肤原样保留，以 `candidate_skin_exists` 拒绝。
 *
 * 失败以 `{code, detail?}` 拒绝：`code` 是 `candidate_skin_*`、`storage` 这类码，`detail` 是清单没通过校验时加载器给出的原因。
 */
export interface CandidateSkinEditorClient {
  check(request: CandidateSkinPackageRequest): Promise<ExternalSkin>;
  create(request: CandidateSkinPackageRequest, replace: boolean): Promise<CreatedCandidateSkin>;
}

/** 预览和预览图用的皮肤：草稿按扫描结果的形状摆出来，图片路径与 `draftImages` 相同。 */
export function draftSkin(draft: CandidateSkinDraft): ExternalSkin {
  const images = new Map(draftImages(draft).map(([path, image]) => [image, path]));
  const palette = (mode: DraftMode): PackageCandidatePalette => {
    const value = draft.palettes[mode];
    return {
      surface: value.surface,
      text: value.text,
      number: value.number,
      translation: value.translation,
      accent: value.accent,
      selected: value.selected,
      hover: value.hover,
      border: value.border,
      showSelectedBar: value.showSelectedBar,
    };
  };
  return {
    id: draft.id || "draft",
    schemaVersion: draftSchemaVersion(draft),
    name: draft.name.trim() || "未命名皮肤",
    version: draft.carried.version,
    base: drawnBase(draft.base),
    author: draft.carried.author,
    description: draft.description.trim() || null,
    layouts: draft.layouts,
    themes: draft.modes,
    minWidthDip: draft.minWidth,
    cornerRadiusDip: draft.cornerRadius,
    decorationTopDip: draft.decoration?.topInset ?? 0,
    decorationWidthDip: draft.decoration?.width ?? 0,
    decorationImage: draft.decoration ? (images.get(draft.decoration.image) ?? null) : null,
    decorationAlign: draft.decoration?.align ?? "right",
    decorationMotion: draft.decoration?.motion ?? "none",
    decorationMotionPeriodMs: draft.decoration?.motionPeriodMs ?? 3000,
    background: draft.background
      ? {
          image: images.get(draft.background.image) ?? "",
          fit: draft.background.fit,
          opacity: draft.background.opacity,
        }
      : null,
    toolbarStylesheet: null,
    preview: null,
    candidate: { light: palette("light"), dark: palette("dark") },
    animation: {
      enter: draft.animation.enabled ? draft.animation.enter : "none",
      exit: draft.animation.enabled ? draft.animation.exit : "none",
      selection: draft.animation.enabled ? draft.animation.selection : "none",
      durationMs: draft.animation.durationMs,
      staggerMs: draft.animation.staggerMs,
    },
  };
}

/** 草稿图片的 `data:` URL。 */
export function draftImageUrl(image: DraftImage): string {
  return `data:${image.contentType};base64,${image.data}`;
}
