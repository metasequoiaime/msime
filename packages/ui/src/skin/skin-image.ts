import { useEffect, useMemo, useState } from "react";
import { validSkinAssetBytes } from "./skin-asset";
import { useAsyncGeneration } from "../settings/use-async-generation";

export type SkinImage = { contentType: string; bytes: number[] };
export type SkinImageReader = (id: string, relative: string) => Promise<SkinImage>;
const imageTypes = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/svg+xml",
  "image/x-icon",
  "image/bmp",
  "image/avif",
]);

// Use an image-only data URL, already permitted by the desktop img-src CSP.
// SVG remains an image, never HTML markup or an executable document.
export function skinImageUrl(image: SkinImage): string {
  if (!imageTypes.has(image.contentType) || !validSkinAssetBytes(image.bytes))
    throw new Error("invalid image");
  let binary = "";
  for (let offset = 0; offset < image.bytes.length; offset += 32768) {
    binary += String.fromCharCode(...image.bytes.slice(offset, offset + 32768));
  }
  return `data:${image.contentType};base64,${btoa(binary)}`;
}

export function useSkinImage(
  read: SkinImageReader | undefined,
  id: string,
  relative: string | null,
  revision: number,
) {
  const [result, setResult] = useState<{ key: object; url?: string; failed?: boolean }>();
  const key = useMemo(() => ({}), [read, id, relative, revision]);
  const generation = useAsyncGeneration(read, id, relative, revision);
  useEffect(() => {
    const current = key;
    const requestGeneration = generation.current;
    setResult(undefined);
    if (read && relative) {
      void (async () => {
        try {
          const image = await read(id, relative);
          if (generation.current === requestGeneration)
            setResult({ key: current, url: skinImageUrl(image) });
        } catch {
          if (generation.current === requestGeneration) setResult({ key: current, failed: true });
        }
      })();
    }
  }, [read, id, relative, key, generation]);
  return result?.key === key ? result : undefined;
}
