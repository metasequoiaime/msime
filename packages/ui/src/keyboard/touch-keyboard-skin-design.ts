// Fixed source: MSIME-Apple@11c950a63ec57656cd78b3f75aa621c293bfe453,
// platforms/ios/SharedUI/CustomKeyboardSkin.swift and KeyboardSkinCollection.swift.
import { boundedGraphemes } from "../core/text";

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

export const touchKeyboardBackgroundPresets: { start: number; end?: number; title: string }[] = [
  { start: 0xffffff, title: "纯白" },
  { start: 0xdfe2eb, title: "雾灰" },
  { start: 0x171717, title: "曜黑" },
  { start: 0xffd5db, title: "樱粉" },
  { start: 0xaee3f3, title: "晴空" },
  { start: 0xc6e9a7, title: "嫩绿" },
  { start: 0x160a3f, end: 0xa747df, title: "紫夜渐变" },
  { start: 0xc5f5ff, end: 0xfad8f6, title: "极光渐变" },
  { start: 0x185c47, end: 0x80bfa8, title: "森林渐变" },
];

export function skinColor(value: number): string {
  return `#${Math.max(0, Math.min(0xffffff, Math.round(value)))
    .toString(16)
    .padStart(6, "0")}`;
}

export function skinLuminance(rgb: number): number {
  const channel = (shift: number) => {
    const value = ((rgb >> shift) & 255) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0);
}

export function skinContrast(first: number, second: number): number {
  const a = skinLuminance(first),
    b = skinLuminance(second);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

export function readableSkinText(background: number): number {
  return skinLuminance(background) > 0.179 ? 0 : 0xffffff;
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
  const clamp = (entry: number, min: number, max: number, fallback: number) =>
    Number.isFinite(entry) ? Math.min(max, Math.max(min, entry)) : fallback;
  return {
    ...value,
    background: color(value.background),
    keyBackground: color(value.keyBackground),
    keyForeground: color(value.keyForeground),
    accent: color(value.accent),
    actionBackground: color(value.actionBackground),
    cornerRadius: clamp(value.cornerRadius, 0, 20, 8),
    borderWidth: clamp(value.borderWidth, 0, 2, 0),
    shadow: clamp(value.shadow, 0, 0.4, 0),
    pattern: value.pattern >= 0 && value.pattern <= 3 ? value.pattern : 0,
    keyOpacity: value.keyOpacity === undefined ? undefined : clamp(value.keyOpacity, 0.25, 1, 1),
    gradientEnd: value.gradientEnd === undefined ? undefined : color(value.gradientEnd),
    patternOpacity:
      value.patternOpacity === undefined ? undefined : clamp(value.patternOpacity, 0, 0.5, 0.15),
    customBorderColor:
      value.customBorderColor === undefined ? undefined : color(value.customBorderColor),
    photoShade: value.photoShade === undefined ? undefined : clamp(value.photoShade, 0, 0.8, 0.25),
    photoPosition:
      value.photoPosition === undefined ? undefined : clamp(value.photoPosition, 0, 1, 0.5),
  };
}
