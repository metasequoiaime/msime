import { bytesToBase64 } from "../core/base64";
import { canvasToBytes } from "../core/canvas-to-bytes";
import { limits, type DraftImage } from "./candidate-skin-draft";

/** 一张图片读不进草稿的原因，已经是给用户看的话。 */
export class DraftImageError extends Error {}

const signatures: [DraftImage["contentType"], (bytes: Uint8Array) => boolean][] = [
  ["image/png", (bytes) => startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])],
  ["image/jpeg", (bytes) => startsWith(bytes, [0xff, 0xd8, 0xff])],
  ["image/gif", (bytes) => startsWith(bytes, [0x47, 0x49, 0x46, 0x38])],
  [
    "image/webp",
    (bytes) =>
      startsWith(bytes, [0x52, 0x49, 0x46, 0x46]) &&
      String.fromCharCode(...bytes.subarray(8, 12)) === "WEBP",
  ],
];

function startsWith(bytes: Uint8Array, prefix: number[]): boolean {
  return prefix.every((byte, index) => bytes[index] === byte);
}

/** 按文件头判断图片类型；扩展名和浏览器给的 MIME 都不可信。 */
export function sniffImageType(bytes: Uint8Array): DraftImage["contentType"] | null {
  return signatures.find(([, matches]) => matches(bytes))?.[0] ?? null;
}

function includesAscii(bytes: Uint8Array, text: string, limit = bytes.length): boolean {
  const codes = [...text].map((character) => character.charCodeAt(0));
  const end = Math.min(limit, bytes.length) - codes.length;
  for (let index = 0; index <= end; index++)
    if (codes.every((code, offset) => bytes[index + offset] === code)) return true;
  return false;
}

/** 图片是不是动图：APNG 在第一个 IDAT 之前有 `acTL`，动画 WebP 带 `ANIM` 块，动画 GIF 带 NETSCAPE 循环扩展。动图不能在页面上缩小重编码，否则只剩第一帧。 */
export function isAnimatedImage(bytes: Uint8Array, type: DraftImage["contentType"]): boolean {
  if (type === "image/png") {
    let offset = 8;
    while (offset + 8 <= bytes.length) {
      const length =
        ((bytes[offset] << 24) |
          (bytes[offset + 1] << 16) |
          (bytes[offset + 2] << 8) |
          bytes[offset + 3]) >>>
        0;
      const name = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
      if (name === "acTL") return true;
      if (name === "IDAT") return false;
      offset += 12 + length;
    }
    return false;
  }
  if (type === "image/webp") return includesAscii(bytes, "ANIM", 4096);
  if (type === "image/gif") return includesAscii(bytes, "NETSCAPE2.0");
  return false;
}

export function base64ToBytes(data: string): Uint8Array {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

async function decode(url: string): Promise<HTMLImageElement> {
  const image = new Image();
  image.src = url;
  try {
    await image.decode();
  } catch {
    throw new DraftImageError("这张图片无法解码，可能已损坏。");
  }
  if (!image.naturalWidth || !image.naturalHeight)
    throw new DraftImageError("这张图片无法解码，可能已损坏。");
  return image;
}

/**
 * 把用户选的图片读进草稿。
 *
 * 按文件头认 PNG（含 APNG）、JPEG、GIF 和 WebP。超过 1 MB 或边长超过 2048 像素的静态图在页面上等比缩小，PNG 仍存成 PNG（保留透明），其余存成 JPEG；动图缩小会丢帧，所以直接请用户换一张。宿主保存时还会按同样的上限逐帧解码再查一遍。
 */
export async function readDraftImage(file: Blob & { name?: string }): Promise<DraftImage> {
  const source = new Uint8Array(await file.arrayBuffer());
  const type = sniffImageType(source);
  if (!type) throw new DraftImageError("只支持 PNG、JPEG、GIF 和 WebP 图片。");
  const fileName = file.name || "image";
  const animated = isAnimatedImage(source, type);
  const data = bytesToBase64(source);
  const image = await decode(`data:${type};base64,${data}`);
  const fits =
    source.length <= limits.imageBytes &&
    image.naturalWidth <= limits.imageSide &&
    image.naturalHeight <= limits.imageSide;
  if (fits)
    return {
      fileName,
      contentType: type,
      data,
      byteLength: source.length,
      width: image.naturalWidth,
      height: image.naturalHeight,
      animated,
    };
  if (animated) throw new DraftImageError("动图超过 1 MB 或边长超过 2048 像素，请先压缩后再选。");
  const output = type === "image/png" ? "image/png" : "image/jpeg";
  let scale = Math.min(1, limits.imageSide / Math.max(image.naturalWidth, image.naturalHeight));
  for (let attempt = 0; attempt < 12; attempt++) {
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
    canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
    const context = canvas.getContext("2d");
    if (!context) throw new DraftImageError("无法压缩这张图片。");
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const bytes = await canvasToBytes(canvas, {
      type: output,
      quality: output === "image/jpeg" ? 0.88 : undefined,
      createError: () => new DraftImageError("无法压缩这张图片。"),
    });
    if (bytes.length <= limits.imageBytes)
      return {
        fileName,
        contentType: output,
        data: bytesToBase64(bytes),
        byteLength: bytes.length,
        width: canvas.width,
        height: canvas.height,
        animated: false,
      };
    scale *= 0.8;
  }
  throw new DraftImageError("这张图片太大，缩小后仍超过 1 MB，请换一张。");
}
