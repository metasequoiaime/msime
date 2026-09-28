import { validSkinAssetBytes } from "./skin-asset";

export type SkinFont = { contentType: string; bytes: number[] };
export type SkinFontReader = (id: string, relative: string) => Promise<SkinFont>;
const fontTypes = new Set(["font/woff", "font/woff2", "font/ttf", "font/otf"]);

// Binary FontFace sources require no URL, network permission or CSP relaxation.
export function skinFontBytes(font: SkinFont): ArrayBuffer {
  if (!fontTypes.has(font.contentType) || !validSkinAssetBytes(font.bytes, true))
    throw new Error("invalid font");
  return Uint8Array.from(font.bytes).buffer;
}
