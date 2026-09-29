/**
 * Bounded view of the user's custom touch-keyboard design, ported from
 * platforms/android/java/app/msime/client/CustomKeyboardSkin.java.
 *
 * Every field arrives from a shared preference document that the host does not control, so each one
 * is clamped or rejected rather than trusted. Photos are decoded here with a small runtime-neutral
 * Base64 reader, keeping the size and magic-number checks testable without a device image decoder.
 */
import { KeyboardGeometry } from '../KeyboardGeometry';

const MAX_PHOTO_BYTES: number = 512000;
const KEY_SHAPES: string[] = ["rounded", "capsule", "ticket", "pebble"];
const KEY_MATERIALS: string[] = ["flat", "raised", "glass", "paper"];

export interface CustomSkinDocument {
  readonly background?: number;
  readonly keyBackground?: number;
  readonly keyForeground?: number;
  readonly accent?: number;
  readonly actionBackground?: number;
  readonly cornerRadius?: number;
  readonly borderWidth?: number;
  readonly shadow?: number;
  readonly pattern?: number;
  readonly monospaced?: boolean;
  readonly keyShape?: string;
  readonly keyMaterial?: string;
  readonly keyOpacity?: number;
  readonly gradientEnd?: number | null;
  readonly gradientHorizontal?: boolean;
  readonly patternOpacity?: number;
  readonly customBorderColor?: number | null;
  readonly photo?: string | null;
  readonly photoShade?: number;
  readonly photoPosition?: number;
}

function bounded(
  value: number | undefined,
  minimum: number,
  maximum: number,
  fallback: number,
): number {
  if (value === undefined || !Number.isFinite(value)) {
    return fallback;
  }
  return KeyboardGeometry.bounded(value, minimum, maximum);
}

function oneOf(value: string | undefined, allowed: string[]): string {
  return value !== undefined && allowed.includes(value) ? value : allowed[0];
}

function color(value: number | undefined, fallback: number): number {
  return value === undefined || !Number.isFinite(value) ? fallback : value & 0xffffff;
}

function hex(value: number): string {
  return "#" + (value & 0xffffff).toString(16).toUpperCase().padStart(6, "0");
}

function startsWith(bytes: Uint8Array, prefix: number[]): boolean {
  if (bytes.length < prefix.length) {
    return false;
  }
  for (let index: number = 0; index < prefix.length; index++) {
    if (bytes[index] !== prefix[index]) {
      return false;
    }
  }
  return true;
}

function decodePhoto(value: string | null | undefined): Uint8Array | null {
  if (
    value === undefined ||
    value === null ||
    value.length === 0 ||
    value.length > 682668 ||
    value.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]*={0,2}$/.test(value)
  )
    return null;
  const alphabet: string = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const padding: number = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
  const size: number = (value.length / 4) * 3 - padding;
  if (size <= 0 || size > MAX_PHOTO_BYTES) return null;
  const output = new Uint8Array(size);
  let cursor: number = 0;
  for (let index: number = 0; index < value.length; index += 4) {
    const a: number = alphabet.indexOf(value[index]);
    const b: number = alphabet.indexOf(value[index + 1]);
    const c: number = value[index + 2] === "=" ? 0 : alphabet.indexOf(value[index + 2]);
    const d: number = value[index + 3] === "=" ? 0 : alphabet.indexOf(value[index + 3]);
    if (a < 0 || b < 0 || c < 0 || d < 0) return null;
    const bits: number = (a << 18) | (b << 12) | (c << 6) | d;
    if (cursor < size) output[cursor++] = (bits >> 16) & 255;
    if (cursor < size) output[cursor++] = (bits >> 8) & 255;
    if (cursor < size) output[cursor++] = bits & 255;
  }
  return output;
}

function photoType(bytes: Uint8Array): string | null {
  if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) {
    return "image/jpeg";
  }
  if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10])) return "image/png";
  if (startsWith(bytes, [0x47, 0x49, 0x46, 0x38])) return "image/gif";
  if (
    bytes.length >= 12 &&
    startsWith(bytes, [0x52, 0x49, 0x46, 0x46]) &&
    bytes[8] === 0x57 &&
    bytes[9] === 0x45 &&
    bytes[10] === 0x42 &&
    bytes[11] === 0x50
  ) {
    return "image/webp";
  }
  return null;
}

/** JPEG, PNG, GIF and WebP by magic number. Anything else is not drawn. */
export function supportedPhoto(bytes: Uint8Array): boolean {
  if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) {
    return true;
  }
  if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10])) {
    return true;
  }
  if (
    startsWith(bytes, [0x47, 0x49, 0x46, 0x38, 0x37, 0x61]) ||
    startsWith(bytes, [0x47, 0x49, 0x46, 0x38, 0x39, 0x61])
  ) {
    return true;
  }
  return (
    bytes.length >= 12 &&
    startsWith(bytes, [0x52, 0x49, 0x46, 0x46]) &&
    bytes[8] === 0x57 &&
    bytes[9] === 0x45 &&
    bytes[10] === 0x42 &&
    bytes[11] === 0x50
  );
}

/** Matches java.util.Arrays.hashCode so the cache key means the same thing on both hosts. */
function photoHash(bytes: Uint8Array | null): number {
  if (bytes === null) {
    return 0;
  }
  let result: number = 1;
  for (let index: number = 0; index < bytes.length; index++) {
    result = (Math.imul(31, result) + bytes[index]) | 0;
  }
  return result;
}

const DEFAULT_BACKGROUND: number = 0xe8f0eb;
const DEFAULT_KEY_BACKGROUND: number = 0xffffff;
const DEFAULT_KEY_FOREGROUND: number = 0x17251d;
const DEFAULT_ACCENT: number = 0x185c47;

/** WCAG relative luminance, which decides whether the action key prints black or white. */
function channel(value: number): number {
  const component: number = (value & 255) / 255;
  return component <= 0.04045 ? component / 12.92 : Math.pow((component + 0.055) / 1.055, 2.4);
}

function luminance(rgb: number): number {
  return 0.2126 * channel(rgb >> 16) + 0.7152 * channel(rgb >> 8) + 0.0722 * channel(rgb);
}

export class CustomKeyboardSkin {
  private backgroundColor: number = DEFAULT_BACKGROUND;
  private keyBackgroundColor: number = DEFAULT_KEY_BACKGROUND;
  private keyForegroundColor: number = DEFAULT_KEY_FOREGROUND;
  private accentColor: number = DEFAULT_ACCENT;
  private actionBackgroundColor: number = DEFAULT_ACCENT;
  private cornerRadiusValue: number = 8;
  private borderWidthValue: number = 0;
  private shadowValue: number = 0;
  private patternValue: number = 0;
  private monospacedValue: boolean = false;
  private keyShapeValue: string = "rounded";
  private keyMaterialValue: string = "flat";
  private keyOpacityValue: number = 1;
  private gradientEndColor: number | null = null;
  private gradientHorizontalValue: boolean = false;
  private patternOpacityValue: number = 0.15;
  private customBorderColor: number | null = null;
  private photoBytes: Uint8Array | null = null;
  private photoSourceValue: string | null = null;
  private photoShadeValue: number = 0.25;
  private photoPositionValue: number = 0.5;

  static defaults(): CustomKeyboardSkin {
    return new CustomKeyboardSkin();
  }

  /** `photo` is the already-decoded image; pass null when there is none or it failed to decode. */
  static from(
    document: CustomSkinDocument | null,
    photo: Uint8Array | null = null,
  ): CustomKeyboardSkin {
    const value: CustomKeyboardSkin = CustomKeyboardSkin.defaults();
    if (document === null) {
      return value;
    }
    value.backgroundColor = color(document.background, value.backgroundColor);
    value.keyBackgroundColor = color(document.keyBackground, value.keyBackgroundColor);
    value.keyForegroundColor = color(document.keyForeground, value.keyForegroundColor);
    value.accentColor = color(document.accent, value.accentColor);
    value.actionBackgroundColor = color(document.actionBackground, value.actionBackgroundColor);
    value.cornerRadiusValue = bounded(document.cornerRadius, 0, 20, 8);
    value.borderWidthValue = bounded(document.borderWidth, 0, 2, 0);
    value.shadowValue = bounded(document.shadow, 0, 0.4, 0);
    value.patternValue = KeyboardGeometry.bounded(document.pattern ?? 0, 0, 3);
    value.monospacedValue = document.monospaced ?? false;
    value.keyShapeValue = oneOf(document.keyShape, KEY_SHAPES);
    value.keyMaterialValue = oneOf(document.keyMaterial, KEY_MATERIALS);
    value.keyOpacityValue = bounded(document.keyOpacity, 0.25, 1, 1);
    value.gradientEndColor =
      document.gradientEnd === undefined || document.gradientEnd === null
        ? null
        : color(document.gradientEnd, value.backgroundColor);
    value.gradientHorizontalValue = document.gradientHorizontal ?? false;
    value.patternOpacityValue = bounded(document.patternOpacity, 0, 0.5, 0.15);
    value.customBorderColor =
      document.customBorderColor === undefined || document.customBorderColor === null
        ? null
        : color(document.customBorderColor, value.accentColor);
    value.photoShadeValue = bounded(document.photoShade, 0, 0.8, 0.25);
    value.photoPositionValue = bounded(document.photoPosition, 0, 1, 0.5);
    const decoded: Uint8Array | null = photo ?? decodePhoto(document.photo);
    value.photoBytes =
      decoded !== null && decoded.length <= MAX_PHOTO_BYTES && supportedPhoto(decoded)
        ? decoded
        : null;
    const mime: string | null = value.photoBytes === null ? null : photoType(value.photoBytes);
    if (
      mime !== null &&
      document.photo !== undefined &&
      document.photo !== null &&
      decoded !== null
    ) {
      value.photoSourceValue = `data:${mime};base64,${document.photo}`;
    }
    return value;
  }

  background(): string {
    return hex(this.backgroundColor);
  }
  keyBackground(): string {
    return hex(this.keyBackgroundColor);
  }
  keyForeground(): string {
    return hex(this.keyForegroundColor);
  }
  accent(): string {
    return hex(this.accentColor);
  }
  actionBackground(): string {
    return hex(this.actionBackgroundColor);
  }

  /** Black on a light action key, white on a dark one, decided by relative luminance. */
  actionForeground(): string {
    return luminance(this.actionBackgroundColor) > 0.179 ? "#000000" : "#FFFFFF";
  }

  /** The shared `on_accent`: text on anything filled with the accent, by the same luminance rule. */
  accentForeground(): string {
    return luminance(this.accentColor) > 0.179 ? "#000000" : "#FFFFFF";
  }

  cornerRadius(): number {
    return this.cornerRadiusValue;
  }
  borderWidth(): number {
    return this.borderWidthValue;
  }
  shadow(): number {
    return this.shadowValue;
  }
  pattern(): number {
    return this.patternValue;
  }
  monospaced(): boolean {
    return this.monospacedValue;
  }
  keyShape(): string {
    return this.keyShapeValue;
  }
  keyMaterial(): string {
    return this.keyMaterialValue;
  }
  keyOpacity(): number {
    return this.keyOpacityValue;
  }
  gradientEnd(): string | null {
    return this.gradientEndColor === null ? null : hex(this.gradientEndColor);
  }
  gradientHorizontal(): boolean {
    return this.gradientHorizontalValue;
  }
  patternOpacity(): number {
    return this.patternOpacityValue;
  }
  borderColor(): string {
    return hex(this.customBorderColor === null ? this.accentColor : this.customBorderColor);
  }
  photo(): Uint8Array | null {
    return this.photoBytes;
  }
  photoSource(): string | null {
    return this.photoSourceValue;
  }
  photoShade(): number {
    return this.photoShadeValue;
  }
  photoPosition(): number {
    return this.photoPositionValue;
  }

  /** Identity for caching a rendered skin. Any field that changes the drawing appears here. */
  key(): string {
    return [
      this.backgroundColor,
      this.keyBackgroundColor,
      this.keyForegroundColor,
      this.accentColor,
      this.actionBackgroundColor,
      this.cornerRadiusValue,
      this.borderWidthValue,
      this.shadowValue,
      this.patternValue,
      this.monospacedValue,
      this.keyShapeValue,
      this.keyMaterialValue,
      this.keyOpacityValue,
      this.gradientEndColor,
      this.gradientHorizontalValue,
      this.patternOpacityValue,
      this.customBorderColor,
      photoHash(this.photoBytes),
      this.photoShadeValue,
      this.photoPositionValue,
    ].join(":");
  }
}
