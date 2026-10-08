import { errorCode } from "../core/error-code";
import type { LocalDictionaryKind } from "./dictionary-file";

export function dictionaryErrorMessage(
  error: unknown,
  fallback: string,
  kind?: LocalDictionaryKind,
): string {
  switch (errorCode(error)) {
    case "dictionary_busy":
      return "词库正在被输入法占用，请关闭正在使用输入法的程序后重试。";
    case "dictionary_import_rejected":
      return "词库拒绝了这次写入，请检查编码与词是否匹配。";
    case "dictionary_too_large":
      return "词库文件过大：文件不能超过 32 MB，单行不能超过 60 KB，请拆分后再导入。";
    case "dictionary_read_rejected":
      return "词库拒绝了这次读取，请稍后重试。";
    case "dictionary_export_limit":
      return "词库导出文件过大，请分批导出。";
    case "dictionary_bundled_readonly":
      return "内置词条只能调整权重或删除，不能修改编码和词。";
    case "dictionary_pinyin_unavailable":
      return "拼音表不可用，无法校验这条词的读音。";
    case "dictionary_reset_rejected":
      return "清除学习数据失败，请关闭正在使用输入法的程序后重试。";
    case "dictionary_unavailable":
      return "无法打开用户词库，请检查输入法是否正在运行。";
    case "dictionary_invalid_entry":
      return invalidDictionaryEntryMessage(kind);
    case "dictionary_invalid_word":
      return "词条内容为空、过长或含控制字符，或权重超出 1 到 100000000 的范围。";
    default:
      return fallback;
  }
}

function invalidDictionaryEntryMessage(kind?: string): string {
  switch (kind) {
    case "pinyin":
      return "拼音必须由完整音节组成，音节数需与汉字数一致，例如“你好”填 nihao 或 ni'hao。";
    case "wubi":
    case "wubi98":
      return "五笔编码须为 1 到 4 个字母。";
    case "quick_phrase":
      return "快捷短语编码只能包含英文字母，长度 1 到 32。";
    case "english":
      return "英文编码只能包含字母、连字符和撇号。";
    default:
      return "词条不符合词库规则，请检查编码与词条后再保存。";
  }
}

export function dictionaryKeyMatches(kind: string, key: string, query: string): boolean {
  if (!query) return true;
  const fold = (text: string) => text.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
  const code = (text: string) => (kind === "pinyin" ? fold(text).replace(/[' ]/g, "") : fold(text));
  return code(kind === "pinyin" ? key : key.trim()).startsWith(code(query));
}

export function importFailureMessage(kind: string, error: unknown): string {
  const reason =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : typeof error === "object" && error !== null && "error" in error
          ? String((error as { error: unknown }).error)
          : "";
  const coded = dictionaryErrorMessage(error, "");
  if (coded) return `${kind}导入失败：${coded}`;
  return reason ? `${kind}导入失败：${reason}` : `${kind}导入失败，请检查文本格式。`;
}
