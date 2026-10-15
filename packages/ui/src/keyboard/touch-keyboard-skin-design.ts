// Fixed source: MSIME-Apple@11c950a63ec57656cd78b3f75aa621c293bfe453,
// platforms/ios/SharedUI/CustomKeyboardSkin.swift and KeyboardSkinCollection.swift.
import { boundedGraphemes } from "../core/text";
import { clamp as clampNumber, clampFinite } from "../core/number";
import { relativeLuminance } from "../core/color";

export { relativeLuminance as skinLuminance };

export function skinColorNumber(value: string): number {
  return Number.parseInt(value.slice(1), 16);
}

export function boundedSkinName(value: string): string {
  return boundedGraphemes(value, 32);
}

export type TouchSkinKeyShape = "rounded" | "capsule" | "ticket" | "pebble";
export type TouchSkinKeyMaterial = "flat" | "raised" | "glass" | "paper";

export type TouchKeyboardSkinDesign = {
  background: number;
  keyBackground: number;
  keyForeground: number;
  accent: number;
  actionBackground: number;
  cornerRadius: number;
  borderWidth: number;
  shadow: number;
  pattern: 0 | 1 | 2 | 3;
  monospaced: boolean;
  keyShape?: TouchSkinKeyShape;
  keyMaterial?: TouchSkinKeyMaterial;
  keyOpacity?: number;
  gradientEnd?: number;
  gradientHorizontal?: boolean;
  patternOpacity?: number;
  customBorderColor?: number;
  /** Base64-encoded bounded image bytes, matching Swift JSONEncoder Data. */
  photo?: string;
  photoShade?: number;
  photoPosition?: number;
};

export type SavedTouchKeyboardSkin = {
  id: string;
  name: string;
  design: TouchKeyboardSkinDesign;
};

export function updateSavedTouchKeyboardSkinName(
  current: SavedTouchKeyboardSkin,
  name: string,
): SavedTouchKeyboardSkin {
  return { ...current, name };
}

export type CustomSkinLibraryAction =
  | { operation: "create"; name: string; design: TouchKeyboardSkinDesign }
  | { operation: "rename"; id: string; name: string }
  | { operation: "update"; id: string; design: TouchKeyboardSkinDesign }
  | { operation: "delete"; id: string };

export type CustomSkinLibraryClient = {
  load(): Promise<SavedTouchKeyboardSkin[]>;
  mutate(action: CustomSkinLibraryAction): Promise<SavedTouchKeyboardSkin[]>;
};

export type AiSkinProposal = {
  name: string;
  description: string;
  design: TouchKeyboardSkinDesign;
  artworkPrompt: string;
  artwork: {
    b64_json: string;
    mime_type: "image/png" | "image/jpeg";
    width: number;
    height: number;
  };
};

export type AiSkinProgress = { requestId: string; completed: number };

export type AiSkinClient = {
  generate(requestId: string, prompt: string): Promise<AiSkinProposal[]>;
  cancel(requestId: string): Promise<void>;
  onProgress?(listener: (progress: AiSkinProgress) => void): Promise<() => void>;
};

export const defaultTouchKeyboardSkinDesign: TouchKeyboardSkinDesign = {
  background: 0xe8f0eb,
  keyBackground: 0xffffff,
  keyForeground: 0x17251d,
  accent: 0x185c47,
  actionBackground: 0x185c47,
  cornerRadius: 8,
  borderWidth: 0,
  shadow: 0,
  pattern: 0,
  monospaced: false,
};

const skin = (patch: Partial<TouchKeyboardSkinDesign>): TouchKeyboardSkinDesign => ({
  ...defaultTouchKeyboardSkinDesign,
  ...patch,
});

export const touchKeyboardSkinTemplates: { title: string; design: TouchKeyboardSkinDesign }[] = [
  // 薄荷晨光（client-core 的 TouchKeyboardSkinDesign::mint_morning），曾是触屏键盘新装的默认皮肤，仍排第一。
  {
    title: "薄荷晨光",
    design: skin({
      background: 0xd8f0e4,
      keyBackground: 0xfafff9,
      keyForeground: 0x173d30,
      accent: 0x245a43,
      actionBackground: 0x245a43,
      cornerRadius: 14,
      borderWidth: 0.5,
      shadow: 0.08,
      gradientEnd: 0xeef6dd,
      customBorderColor: 0xb6d8c5,
    }),
  },
  {
    title: "苔庭晨雾",
    design: skin({
      background: 0xe0e9df,
      keyBackground: 0xf7faf3,
      keyForeground: 0x243f32,
      accent: 0x214d3a,
      actionBackground: 0x2f6047,
      cornerRadius: 12,
      borderWidth: 0.5,
      shadow: 0.1,
      pattern: 3,
      gradientEnd: 0xc6d9ca,
      gradientHorizontal: true,
      patternOpacity: 0.035,
      customBorderColor: 0xb8cdbe,
    }),
  },
  {
    title: "竹影青瓷",
    design: skin({
      background: 0xd9e8e2,
      keyBackground: 0xf5f8ee,
      keyForeground: 0x243f38,
      accent: 0x265443,
      actionBackground: 0x265443,
      cornerRadius: 4,
      borderWidth: 1,
      shadow: 0.04,
      gradientEnd: 0xebf2e7,
      gradientHorizontal: true,
      patternOpacity: 0,
      customBorderColor: 0x94b4a3,
    }),
  },
  {
    title: "月下银砂",
    design: skin({
      background: 0x181f2b,
      keyBackground: 0x303e4f,
      keyForeground: 0xeff5fc,
      accent: 0xcee0f3,
      actionBackground: 0xcadbec,
      cornerRadius: 10,
      borderWidth: 0.5,
      shadow: 0.08,
      pattern: 1,
      gradientEnd: 0x283645,
      gradientHorizontal: true,
      patternOpacity: 0.07,
      customBorderColor: 0x6f8399,
    }),
  },
  {
    title: "黑金刻度",
    design: skin({
      background: 0x191b19,
      keyBackground: 0x292d29,
      keyForeground: 0xefe9d5,
      accent: 0xe1cc91,
      actionBackground: 0xdac486,
      cornerRadius: 3,
      borderWidth: 0.75,
      pattern: 2,
      monospaced: true,
      gradientEnd: 0x202720,
      gradientHorizontal: true,
      patternOpacity: 0.04,
      customBorderColor: 0x8d8058,
    }),
  },
  {
    title: "樱雪糯米",
    design: skin({
      background: 0xf4dfe5,
      keyBackground: 0xfff8f6,
      keyForeground: 0x503449,
      accent: 0x733e58,
      actionBackground: 0x904d69,
      cornerRadius: 18,
      shadow: 0.14,
      pattern: 3,
      gradientEnd: 0xe7e2f2,
      gradientHorizontal: true,
      patternOpacity: 0.04,
      customBorderColor: 0xdfbbc9,
    }),
  },
  {
    title: "落日陶土",
    design: skin({
      background: 0xead4c4,
      keyBackground: 0xfff4df,
      keyForeground: 0x56382c,
      accent: 0x733f2b,
      actionBackground: 0x9a4e32,
      cornerRadius: 7,
      borderWidth: 0.75,
      shadow: 0.18,
      pattern: 1,
      gradientEnd: 0xf3e4d1,
      gradientHorizontal: true,
      patternOpacity: 0.05,
      customBorderColor: 0xcba78d,
    }),
  },
  {
    title: "冰川薄荷",
    design: skin({
      background: 0xd9ebea,
      keyBackground: 0xf5ffff,
      keyForeground: 0x203e4b,
      accent: 0x275360,
      actionBackground: 0x34717c,
      cornerRadius: 14,
      borderWidth: 0.5,
      shadow: 0.06,
      pattern: 3,
      gradientEnd: 0xdde7f4,
      gradientHorizontal: true,
      patternOpacity: 0.035,
      customBorderColor: 0xc0dcdb,
    }),
  },
  {
    title: "奶咖手账",
    design: skin({
      background: 0xd9cfc0,
      keyBackground: 0xf6efe2,
      keyForeground: 0x453b31,
      accent: 0x5a4630,
      actionBackground: 0x65523b,
      cornerRadius: 5,
      borderWidth: 1,
      shadow: 0.2,
      pattern: 2,
      monospaced: true,
      gradientEnd: 0xe8dfd0,
      gradientHorizontal: true,
      patternOpacity: 0.06,
      customBorderColor: 0xb09b83,
    }),
  },
  { title: "水杉留白", design: skin({}) },
  {
    title: "复古纸感",
    design: skin({
      background: 0xe3d6bd,
      keyBackground: 0xfff5df,
      keyForeground: 0x382a1c,
      accent: 0x53391f,
      actionBackground: 0x53391f,
      cornerRadius: 4,
      borderWidth: 1,
      shadow: 0.3,
      monospaced: true,
      pattern: 1,
      keyShape: "ticket",
      keyMaterial: "paper",
    }),
  },
  {
    title: "紫夜星光",
    design: skin({
      background: 0x151022,
      gradientEnd: 0x30224a,
      keyBackground: 0x291e40,
      keyForeground: 0xffffff,
      accent: 0xd4bbff,
      actionBackground: 0x69469b,
      borderWidth: 1,
      customBorderColor: 0xa987e8,
      pattern: 1,
      keyShape: "rounded",
      keyMaterial: "glass",
    }),
  },
  {
    title: "奶油桃桃",
    design: skin({
      background: 0xffe0d0,
      gradientEnd: 0xf9d6e5,
      keyBackground: 0xfff8ee,
      keyForeground: 0x51283a,
      accent: 0x84334f,
      actionBackground: 0x84334f,
      cornerRadius: 18,
      shadow: 0.15,
      pattern: 3,
      keyShape: "pebble",
      keyMaterial: "raised",
    }),
  },
  {
    title: "海盐渐变",
    design: skin({
      background: 0xdceaf8,
      gradientEnd: 0xddefe9,
      gradientHorizontal: true,
      accent: 0x224e75,
      actionBackground: 0x224e75,
      borderWidth: 0.5,
    }),
  },
  {
    title: "工程蓝图",
    design: skin({
      background: 0x102438,
      keyBackground: 0x17354f,
      accent: 0xa2d8fa,
      actionBackground: 0x285d84,
      cornerRadius: 2,
      borderWidth: 1,
      monospaced: true,
      pattern: 2,
      customBorderColor: 0x548caa,
      keyForeground: 0xffffff,
      keyShape: "rounded",
      keyMaterial: "glass",
    }),
  },
];

/**
 * 编辑器「背景」里的预设。每个预设带一整套配色，而不只是背景：按键文字色同时画在按键上和背景上（工具栏图标、候选栏文字都直接压在背景上），只换背景时，深色背景配默认的白键深字，工具栏就看不见了，默认的绿色强调色在紫色、粉色背景上也很突兀。每一套都满足 `hasReadableSkinText`，按键文字色在背景（渐变的两端）上的对比度也不低于 3:1。
 */
export const touchKeyboardBackgroundPresets: {
  title: string;
  start: number;
  end?: number;
  keyBackground: number;
  keyForeground: number;
  accent: number;
  actionBackground: number;
}[] = [
  {
    title: "纯白",
    start: 0xffffff,
    keyBackground: 0xf1f3f5,
    keyForeground: 0x1f2329,
    accent: 0x185c47,
    actionBackground: 0x185c47,
  },
  {
    title: "雾灰",
    start: 0xdfe2eb,
    keyBackground: 0xffffff,
    keyForeground: 0x1f2329,
    accent: 0x2f4a7a,
    actionBackground: 0x2f4a7a,
  },
  {
    title: "曜黑",
    start: 0x171717,
    keyBackground: 0x2e2e30,
    keyForeground: 0xf2f2f2,
    accent: 0x8fd5a6,
    actionBackground: 0x2c7a4b,
  },
  {
    title: "樱粉",
    start: 0xffd5db,
    keyBackground: 0xffffff,
    keyForeground: 0x3d1f27,
    accent: 0xa3304f,
    actionBackground: 0xa3304f,
  },
  {
    title: "晴空",
    start: 0xaee3f3,
    keyBackground: 0xffffff,
    keyForeground: 0x14324a,
    accent: 0x1d5f8a,
    actionBackground: 0x1d5f8a,
  },
  {
    title: "嫩绿",
    start: 0xc6e9a7,
    keyBackground: 0xffffff,
    keyForeground: 0x1f3317,
    accent: 0x3a6619,
    actionBackground: 0x3a6619,
  },
  {
    title: "紫夜渐变",
    start: 0x160a3f,
    end: 0xa747df,
    keyBackground: 0x2e1f63,
    keyForeground: 0xffffff,
    accent: 0xffffff,
    actionBackground: 0x8b5cf6,
  },
  {
    title: "极光渐变",
    start: 0xc5f5ff,
    end: 0xfad8f6,
    keyBackground: 0xffffff,
    keyForeground: 0x2a2f45,
    accent: 0x5b3fb0,
    actionBackground: 0x5b3fb0,
  },
  {
    title: "森林渐变",
    start: 0x0f3d2f,
    end: 0x35806a,
    keyBackground: 0x0f4a38,
    keyForeground: 0xffffff,
    accent: 0xffffff,
    actionBackground: 0x9be3c0,
  },
];

export function skinColor(value: number): string {
  return `#${clampNumber(Math.round(value), 0, 0xffffff).toString(16).padStart(6, "0")}`;
}

export function skinContrast(first: number, second: number): number {
  const a = relativeLuminance(first),
    b = relativeLuminance(second);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

export function readableSkinText(background: number): number {
  return relativeLuminance(background) > 0.179 ? 0 : 0xffffff;
}

export function hasReadableSkinText(design: TouchKeyboardSkinDesign): boolean {
  return (
    skinContrast(design.keyForeground, design.keyBackground) >= 4.5 &&
    skinContrast(design.accent, design.background) >= 4.5 &&
    skinContrast(design.accent, design.keyBackground) >= 4.5 &&
    (design.gradientEnd === undefined || skinContrast(design.accent, design.gradientEnd) >= 4.5)
  );
}

export function normalizeTouchKeyboardSkinDesign(
  value: TouchKeyboardSkinDesign,
): TouchKeyboardSkinDesign {
  const color = (entry: number) => Math.round(entry) & 0xffffff;
  return {
    ...value,
    background: color(value.background),
    keyBackground: color(value.keyBackground),
    keyForeground: color(value.keyForeground),
    accent: color(value.accent),
    actionBackground: color(value.actionBackground),
    cornerRadius: clampFinite(value.cornerRadius, 0, 20, 8),
    borderWidth: clampFinite(value.borderWidth, 0, 2, 0),
    shadow: clampFinite(value.shadow, 0, 0.4, 0),
    pattern: value.pattern >= 0 && value.pattern <= 3 ? value.pattern : 0,
    keyOpacity:
      value.keyOpacity === undefined ? undefined : clampFinite(value.keyOpacity, 0.25, 1, 1),
    gradientEnd: value.gradientEnd === undefined ? undefined : color(value.gradientEnd),
    patternOpacity:
      value.patternOpacity === undefined
        ? undefined
        : clampFinite(value.patternOpacity, 0, 0.5, 0.15),
    customBorderColor:
      value.customBorderColor === undefined ? undefined : color(value.customBorderColor),
    photoShade:
      value.photoShade === undefined ? undefined : clampFinite(value.photoShade, 0, 0.8, 0.25),
    photoPosition:
      value.photoPosition === undefined ? undefined : clampFinite(value.photoPosition, 0, 1, 0.5),
  };
}
