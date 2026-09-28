export const TOUCH_SKIN_IMAGE_MAX_BYTES = 512_000;

export async function readAndCompressTouchSkinPhoto(file: File): Promise<string> {
  if (!file.type.startsWith("image/") || file.size > 20_000_000) throw new Error("invalid image");
  const url = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("image read failed"));
    reader.readAsDataURL(file);
  });
  return compressTouchSkinImage(url, "image decode failed", "image too large");
}

export async function boundTouchSkinArtwork(artwork: {
  b64_json: string;
  mime_type: string;
}): Promise<string> {
  const source = artwork.b64_json;
  const bytes = Uint8Array.from(atob(source), (character) => character.charCodeAt(0));
  if (bytes.length > TOUCH_SKIN_IMAGE_MAX_BYTES) {
    return compressTouchSkinImage(
      `data:${artwork.mime_type};base64,${source}`,
      "artwork decode failed",
      "artwork too large",
    );
  }
  return source;
}

export async function compressTouchSkinImage(
  url: string,
  decodeError: string,
  sizeError: string,
): Promise<string> {
  const image = new Image();
  image.src = url;
  await new Promise<void>((resolve, reject) => {
    image.onload = () => resolve();
    image.onerror = () => reject(new Error(decodeError));
  });
  if (!image.naturalWidth || !image.naturalHeight) throw new Error("empty image");
  const scale = Math.min(1, 1024 / Math.max(image.naturalWidth, image.naturalHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("canvas unavailable");
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  for (const quality of [0.8, 0.6, 0.4, 0.2]) {
    const data = canvas.toDataURL("image/jpeg", quality).split(",")[1] ?? "";
    if (Math.floor(data.length * 0.75) <= TOUCH_SKIN_IMAGE_MAX_BYTES) return data;
  }
  throw new Error(sizeError);
}
