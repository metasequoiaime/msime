import type { DictionaryImportResult } from "./dictionary-contracts";

export type ImportSummary = Omit<DictionaryImportResult, "failed"> & {
  failed?: DictionaryImportResult["failed"];
};

/** 导入完成的提示。`source` 是放在最前面的一句来源说明，例如细胞词库的「已读取搜狗细胞词库「…」的 n 个词条。」。 */
export function describeImportResult(kind: string, result: ImportSummary, source = ""): string {
  const parts = [`${source}${kind}导入完成，共 ${result.applied} 条。`];
  if (result.failed) {
    const failures = result.first_failures ?? [];
    const lines = failures.map((failure) => failure.line).join("、");
    parts.push(
      lines ? `跳过 ${result.failed} 行，首先出现在第 ${lines} 行。` : `跳过 ${result.failed} 行。`,
    );
    if (failures.some((failure) => failure.issue === "rejected")) {
      parts.push("其中部分行的编码与词不匹配，例如简拼、或音节数与汉字数不一致。");
    }
  }
  if (result.truncated) parts.push("文件过长，仅导入了前一部分。");
  if (result.swapped) parts.push("该文件的两列与所选格式相反，已按文件本身的顺序读取。");
  return parts.join("");
}

/** 细胞词库导入成功后接在提示末尾的一句：导入的词只给用户自己在本机用，社区发布会拦下它们（`thirdPartyMatches`）。原因见 `.agents/notes/implemented/feature/2026-10-17-scel-community-publish-guard.md`。 */
export const SCEL_PERSONAL_USE_NOTICE =
  "搜狗细胞词库的整理成果属于原作者，导入的词只供你在本机使用，不能发布到社区。";

/** 细胞词库导入提示的来源句。 */
export function scelImportSource(name: string, entries: number): string {
  return name
    ? `已读取搜狗细胞词库「${name}」的 ${entries} 个词条。`
    : `已读取搜狗细胞词库的 ${entries} 个词条。`;
}

export function dictionaryKindKeyHint(kind: string): string {
  switch (kind) {
    case "wubi":
    case "wubi98":
      return "1–4 个字母";
    case "quick_phrase":
      return "1–32 个字母";
    case "english":
      return "1–64 个字母";
    case "pinyin":
      return "完整音节，用 ' 分隔，如 ni'hao";
    case "cantonese":
      return "粤拼，每字一个音节，用空格分隔，如 nei hou";
    default:
      return "";
  }
}

export function personalDictionaryKindTitle(
  kind: "pinyin" | "wubi" | "wubi98" | "quickPhrase" | "english" | "cantonese",
): string {
  return kind === "pinyin"
    ? "拼音"
    : kind === "wubi"
      ? "86 五笔"
      : kind === "wubi98"
        ? "98 五笔"
        : kind === "quickPhrase"
          ? "快捷短语"
          : kind === "cantonese"
            ? "粤语"
            : "英文";
}
