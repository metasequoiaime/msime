// Fixed source: MSIME-Apple@11c950a63ec57656cd78b3f75aa621c293bfe453,
// platforms/ios/SharedUI/CustomKeyboardSkin.swift and KeyboardSkinCollection.swift.
import skinPresets from "../../../../resources/keyboard-skin-presets.json";
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

// 网页与原生宿主共用固定目录；Rust 检查默认设计与共享偏好一致。
export const defaultTouchKeyboardSkinDesign: TouchKeyboardSkinDesign =
  skinPresets.default as TouchKeyboardSkinDesign;

export const touchKeyboardSkinTemplates = skinPresets.templates as {
  title: string;
  design: TouchKeyboardSkinDesign;
}[];

/** 背景预设同时调整按键与文字配色，保证工具栏和候选文字可读。 */
export const touchKeyboardBackgroundPresets: {
  title: string;
  start: number;
  end?: number;
  keyBackground: number;
  keyForeground: number;
  accent: number;
  actionBackground: number;
}[] = skinPresets.backgrounds;

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
