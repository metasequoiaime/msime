import { bytesToBase64 } from "../core/base64";
import {
  draftImages,
  draftManifest,
  draftSkin,
  type CandidateSkinDraft,
  type CandidateSkinPackageRequest,
} from "./candidate-skin-draft";
import { base64ToBytes } from "./candidate-skin-images";
import type { ExternalSkin } from "./external-skins";
import type { SkinImageReader } from "./skin-image";
import { renderSkinPreview } from "./skin-preview-render";

/** 画包的预览图：与发布对话框给没有预览图的包补画时同一个 `renderSkinPreview`。测试里换成不需要 canvas 的替身。 */
export type PreviewRenderer = (skin: ExternalSkin, read: SkinImageReader) => Promise<number[]>;

/**
 * 把草稿打成交给宿主的包：先按草稿画出预览图（PNG，超过 256 KB 时是 JPEG），再写出引用它的清单，最后带上背景图和装饰图。
 *
 * 预览图画不出来时以 `{code: "preview_failed"}` 拒绝，页面照常显示错误。
 */
export async function buildCandidateSkinPackage(
  draft: CandidateSkinDraft,
  render: PreviewRenderer = renderSkinPreview,
): Promise<CandidateSkinPackageRequest> {
  const images = draftImages(draft);
  const byPath = new Map(images);
  const read: SkinImageReader = async (_id, path) => {
    const image = byPath.get(path);
    if (!image) throw new Error("missing image");
    return { contentType: image.contentType, bytes: Array.from(base64ToBytes(image.data)) };
  };
  let preview: number[];
  try {
    preview = await render(draftSkin(draft), read);
  } catch {
    throw { code: "preview_failed" };
  }
  const name = preview[0] === 0xff && preview[1] === 0xd8 ? "preview.jpg" : "preview.png";
  const files: Record<string, string> = { [name]: bytesToBase64(preview) };
  for (const [path, image] of images) files[path] = image.data;
  return { packageId: draft.id, manifest: draftManifest(draft, name), files };
}
