import { errorCode } from "../core/error-code";
import { DraftImageError } from "./candidate-skin-images";

/** 皮肤目录加载器（`catalog::load`）给出的原因，换成用户能照着改的话。没列出的原因原样附在后面，免得把具体问题吞掉。 */
const reasons: [RegExp, string][] = [
  [/^name has invalid length$/, "名称不能为空，最多 80 个字节。"],
  [/^description has invalid length$/, "说明最多 500 个字节。"],
  [/^invalid supports$|^missing supports$/, "至少要选一种候选排列和一种明暗模式。"],
  [/^invalid min_width_dip$/, "最小宽度要在 0 到 1000 之间。"],
  [/^invalid corner_radius_dip$/, "圆角要在 0 到 32 之间。"],
  [/^invalid decoration$/, "装饰图的设置无效：顶部留白和宽度都要大于 0，并且要选一张图片。"],
  [/decoration motion/, "装饰图的待机动效无效：周期要在 1200 到 8000 毫秒之间。"],
  [/^invalid background$/, "背景图的设置无效：不透明度要在 0 到 1 之间。"],
  [/candidate colors|candidate color exceeds/, "有颜色的写法无效，请用 #RRGGBB 或 #RRGGBBAA。"],
  [/^invalid animation/, "动效设置无效：时长 40–600 毫秒，逐行间隔 0–80 毫秒。"],
  [
    /^image dimensions exceed limits$/,
    "图片太大：每边最多 2048 像素，全部图片合计最多 800 万像素。",
  ],
  [/^animated image has too many frames$/, "动图最多 120 帧。"],
  [/^image is too large$|^image is unavailable$/, "有图片读不出来或超过大小上限。"],
  [/^invalid skin id$|^manifest id does not match folder$/, "皮肤 ID 不可用。"],
];

export function manifestReasonMessage(reason: string): string {
  const known = reasons.find(([pattern]) => pattern.test(reason));
  return known ? known[1] : `清单里有无效的设置（${reason}）。`;
}

/** 保存失败时显示的话。宿主以 `{code, detail?}` 拒绝，`detail` 是清单没通过校验的具体原因。 */
export function candidateSkinSaveMessage(error: unknown): string {
  if (error instanceof DraftImageError) return error.message;
  const detail =
    typeof error === "object" && error !== null && "detail" in error
      ? (error as { detail: unknown }).detail
      : null;
  switch (errorCode(error)) {
    case "candidate_skin_id":
      return "皮肤 ID 不可用：只能用小写英文字母、数字和 . _ -，以字母或数字开头，也不能与内置主题重名。";
    case "candidate_skin_package":
      return typeof detail === "string" && detail
        ? `皮肤没有通过校验：${manifestReasonMessage(detail)}`
        : "皮肤没有通过校验：包里的图片要和清单引用的完全一致，并带一张 PNG 或 JPEG 预览图。";
    case "candidate_skin_file_type":
      return "图片格式不受支持：只能用 PNG、JPEG、GIF 和 WebP，预览图须为 PNG 或 JPEG。";
    case "candidate_skin_file_path":
      return "图片的文件名或数量不符合要求。";
    case "candidate_skin_too_large":
      return "图片太大：每张最多 1 MB、每边最多 2048 像素，合计最多 2 MB，预览图最多 256 KB。";
    case "candidate_skin_image_invalid":
      return "有图片已损坏或无法解码，请换一张再试。";
    case "candidate_skin_exists":
      return "本地已有同 ID 的皮肤。";
    case "storage":
      return "无法写入皮肤目录，请检查数据目录的权限后重试。";
    case "preview_failed":
      return "无法生成预览图，请重试。";
  }
  return "保存失败，请重试。";
}
