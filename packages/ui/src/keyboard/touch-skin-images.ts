export const TOUCH_SKIN_IMAGE_MAX_BYTES = 512_000;

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
