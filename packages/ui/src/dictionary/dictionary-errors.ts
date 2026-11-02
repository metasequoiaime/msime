import { errorCode } from "../core/error-code";
import { asciiLowercase } from "../core/text";
import type { DictionaryEntry, LocalDictionaryKind } from "./dictionary-file";

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
    case "dictionary_scel_unsupported":
      return "这个文件不是能识别的搜狗细胞词库（.scel）。";
    case "dictionary_scel_malformed":
      return "细胞词库文件不完整或已损坏，请重新下载后再导入。";
    case "dictionary_scel_empty":
      return "细胞词库里没有词条。";
    case "dictionary_scel_invalid":
      return "细胞词库读取失败，请重新选择文件。";
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
    case "cantonese":
      return "粤语编码要写成不带声调的粤拼，每个字一个音节、用空格分开，音节数需与字数一致，例如“你好”填 nei hou。";
    default:
      return "词条不符合词库规则，请检查编码与词条后再保存。";
  }
}

export function dictionaryKeyMatches(kind: string, key: string, query: string): boolean {
  if (!query) return true;
  // 拼音和粤语的编码按音节分开存，搜索时不必打出分隔。
  const syllabic = kind === "pinyin" || kind === "cantonese";
  const code = (text: string) =>
    syllabic ? asciiLowercase(text).replace(/[' ]/g, "") : asciiLowercase(text);
  return code(syllabic ? key : key.trim()).startsWith(code(query));
}

/** 兼容旧宿主未筛选的响应；词条查询不再误用编码前缀规则。 */
export function dictionaryEntryMatches(entry: DictionaryEntry, query: string): boolean {
  if (/[^\p{ASCII}]/u.test(query)) {
    const word = query.trim();
    return word.length > 0 && entry.value.includes(word);
  }
  return dictionaryKeyMatches(entry.kind, entry.key, query);
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
