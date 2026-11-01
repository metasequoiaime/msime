import { downloadBlob } from "../core/download-blob";
import { formatZhNumber } from "../core/format-number";
import type { DictionaryClient } from "./dictionary-contracts";
import {
  dictionaryExportName,
  dictionaryExportPayload,
  loadDictionaryExportText,
} from "./dictionary-export";

/** 「导出词库」：分页读出自己加过的拼音词条（`kind` 为 `cantonese` 时是粤语词条），交给宿主的保存对话框；返回要提示的话，用户取消时返回空串。 */
export async function exportPersonalPinyinDictionary(
  dictionary: Pick<DictionaryClient, "export">,
  saveExport?: (name: string, contents: string) => Promise<string | null>,
  kind: "pinyin" | "cantonese" = "pinyin",
): Promise<string> {
  if (!dictionary.export) return "";
  try {
    const text = await loadDictionaryExportText(dictionary, kind, "standard");
    const payload = dictionaryExportPayload(kind, "standard", text);
    if (!payload.rows)
      return kind === "cantonese" ? "还没有自己加过的粤语词条。" : "还没有自己加过的拼音词条。";
    const name = dictionaryExportName(kind);
    if (saveExport) {
      const path = await saveExport(name, payload.body);
      return path === null ? "" : `已导出 ${formatZhNumber(payload.rows)} 条`;
    }
    downloadBlob(new Blob([payload.body], { type: "text/plain;charset=utf-8" }), name);
    return `已导出 ${formatZhNumber(payload.rows)} 条`;
  } catch (error) {
    return error instanceof Error && error.message && !error.message.startsWith("dictionary_")
      ? error.message
      : "词库导出失败，请稍后重试。";
  }
}
