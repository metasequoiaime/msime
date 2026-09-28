export type ImportSummary = {
  applied: number;
  failed?: number;
  truncated?: boolean;
  swapped?: boolean;
  first_failures?: { line: number; issue: string }[];
};

export function describeImportResult(kind: string, result: ImportSummary): string {
  const parts = [`${kind}导入完成，共 ${result.applied} 条。`];
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

export function dictionaryKindKeyHint(kind: string): string {
  switch (kind) {
    case "wubi":
      return "1–4 个字母";
    case "quick_phrase":
      return "1–32 个字母";
    case "english":
      return "1–64 个字母";
    case "pinyin":
      return "完整音节，用 ' 分隔，如 ni'hao";
    default:
      return "";
  }
}

export function personalDictionaryKindTitle(
  kind: "pinyin" | "wubi" | "quickPhrase" | "english",
): string {
  return kind === "pinyin"
    ? "拼音"
    : kind === "wubi"
      ? "五笔"
      : kind === "quickPhrase"
        ? "快捷短语"
        : "英文";
}
