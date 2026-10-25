import { customCandidatePalette } from "../theme/global-theme";
import { canvasToBytes } from "../core/canvas-to-bytes";
import { previewSamples } from "./skin-candidate-preview";
import type { ExternalSkin } from "./external-skins";
import { skinImageUrl, type SkinImageReader } from "./skin-image";

/** 社区的预览图大小上限（client-services 中的 `MAX_PREVIEW_BYTES`）。 */
const previewLimit = 256 * 1024;
/** Pixels per dip, so the gallery's cards stay sharp on a high-density screen. */
const scale = 2;
const padding = 16;
const cardHeight = 68;
const font = `system-ui, -apple-system, "PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", sans-serif`;

async function loadImage(
  read: SkinImageReader,
  id: string,
  path: string | null | undefined,
): Promise<HTMLImageElement | null> {
  if (!path) return null;
  try {
    const image = new Image();
    image.src = skinImageUrl(await read(id, path));
    await image.decode();
    return image;
  } catch {
    // A missing or unreadable image leaves its part of the preview out rather than costing the whole preview.
    return null;
  }
}

/** Where `image` goes to fill a `width` by `height` box the way the manifest's `fit` asks. */
function fitted(
  image: HTMLImageElement,
  fit: "cover" | "contain" | "stretch",
  width: number,
  height: number,
): [number, number, number, number] {
  if (fit === "stretch") return [0, 0, width, height];
  const ratio = (fit === "cover" ? Math.max : Math.min)(
    width / image.naturalWidth,
    height / image.naturalHeight,
  );
  const w = image.naturalWidth * ratio;
  const h = image.naturalHeight * ratio;
  return [(width - w) / 2, (height - h) / 2, w, h];
}

/**
 * A preview image for a package that has none: its candidate window in its own colours, background and decoration, holding the same sample candidates as the settings preview. A PNG, or a JPEG when the PNG would exceed the community's preview limit.
 *
 * Drawn in the light mode when the package supports it, since most gallery cards sit on a light page.
 */
export async function renderSkinPreview(
  skin: ExternalSkin,
  read: SkinImageReader,
): Promise<number[]> {
  const mode = skin.themes.includes("light") || !skin.themes.includes("dark") ? "light" : "dark";
  const palette = customCandidatePalette(skin.base, undefined, skin.candidate[mode]);
  const light = mode === "light";
  const surface = palette?.surface ?? (light ? "#FFFFFF" : "#202020");
  const text = palette?.text ?? (light ? "#1F1F1F" : "#F2F2F2");
  const number = palette?.number ?? text;
  const accent = palette?.accent ?? "#0067C0";
  const decorated =
    skin.decorationTopDip > 0 && skin.decorationWidthDip > 0 && Boolean(skin.decorationImage);
  const [decoration, background] = await Promise.all([
    decorated ? loadImage(read, skin.id, skin.decorationImage) : null,
    loadImage(read, skin.id, skin.background?.image),
  ]);
  const band = decoration ? skin.decorationTopDip : 0;
  const cardWidth = Math.max(skin.minWidthDip, 340);
  const radius = skin.cornerRadiusDip ?? 8;

  const canvas = document.createElement("canvas");
  canvas.width = (cardWidth + padding * 2) * scale;
  canvas.height = (band + cardHeight + padding * 2) * scale;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("canvas unavailable");
  context.scale(scale, scale);
  context.fillStyle = light ? "#E9EDF3" : "#16181D";
  context.fillRect(0, 0, canvas.width, canvas.height);

  if (decoration) {
    const width = skin.decorationWidthDip;
    const x =
      skin.decorationAlign === "left"
        ? padding + radius
        : skin.decorationAlign === "center"
          ? padding + (cardWidth - width) / 2
          : padding + cardWidth - radius - width;
    context.drawImage(decoration, x, padding, width, band);
  }

  const top = padding + band;
  context.save();
  context.beginPath();
  context.roundRect(padding, top, cardWidth, cardHeight, radius);
  context.fillStyle = surface;
  context.fill();
  context.clip();
  if (background && skin.background) {
    const [x, y, w, h] = fitted(background, skin.background.fit, cardWidth, cardHeight);
    context.globalAlpha = skin.background.opacity;
    context.drawImage(background, padding + x, top + y, w, h);
    context.globalAlpha = 1;
  }
  context.restore();
  if (palette?.border) {
    context.beginPath();
    context.roundRect(padding + 0.5, top + 0.5, cardWidth - 1, cardHeight - 1, radius);
    context.strokeStyle = palette.border;
    context.lineWidth = 1;
    context.stroke();
  }

  context.textBaseline = "middle";
  context.font = `13px ${font}`;
  context.fillStyle = palette?.secondary ?? number;
  context.fillText("ni'mf", padding + 12, top + 18);

  // One horizontal row of candidates, the first selected, as a user sees it while typing.
  let x = padding + 8;
  const rowTop = top + 32;
  const rowHeight = 28;
  for (const [index, [candidate]] of previewSamples.slice(0, 5).entries()) {
    context.font = `12px ${font}`;
    const label = `${index + 1}`;
    const labelWidth = context.measureText(label).width;
    context.font = `16px ${font}`;
    const width = labelWidth + 4 + context.measureText(candidate).width + 16;
    if (x + width > padding + cardWidth - 8) break;
    const selected = index === 0;
    if (selected) {
      context.beginPath();
      context.roundRect(x, rowTop, width, rowHeight, Math.min(6, radius));
      context.fillStyle = palette?.selected ?? `${accent.slice(0, 7)}24`;
      context.fill();
    }
    context.font = `12px ${font}`;
    context.fillStyle = (selected && palette?.selected_number) || number;
    context.fillText(label, x + 8, rowTop + rowHeight / 2);
    context.font = `16px ${font}`;
    context.fillStyle = (selected && palette?.selected_text) || text;
    context.fillText(candidate, x + 8 + labelWidth + 4, rowTop + rowHeight / 2);
    x += width + 4;
  }

  const createError = () => new Error("preview encoding failed");
  const png = await canvasToBytes(canvas, { type: "image/png", createError });
  if (png.length <= previewLimit) return Array.from(png);
  const jpeg = await canvasToBytes(canvas, { type: "image/jpeg", quality: 0.85, createError });
  if (jpeg.length <= previewLimit) return Array.from(jpeg);
  throw new Error("preview too large");
}
