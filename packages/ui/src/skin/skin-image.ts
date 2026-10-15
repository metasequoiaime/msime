import { useEffect, useMemo, useState } from "react";
import { bytesToBase64 } from "../core/base64";
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

// 仅生成桌面 img-src CSP 已允许的图片数据 URL；SVG 也只作为图片，不作为 HTML 或可执行文档。
export function skinImageUrl(image: SkinImage): string {
  if (!imageTypes.has(image.contentType) || !validSkinAssetBytes(image.bytes))
    throw new Error("invalid image");
  return `data:${image.contentType};base64,${bytesToBase64(image.bytes)}`;
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
