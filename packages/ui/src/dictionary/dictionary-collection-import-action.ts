import { exceptionMessage } from "../core/exception-message";
import {
  binaryCollectionFormat,
  collectionFailureMessage,
  collectionFormatForFile,
  collectionImportMessage,
  collectionNameFromFile,
  type CollectionImportSource,
  type DictionaryCollectionsClient,
  type DictionaryCollectionsView,
} from "./dictionary-collections";
import { MAX_SCEL_FILE_BYTES, readDictionaryFile, readScelFile } from "./dictionary-file";
import { SCEL_PERSONAL_USE_NOTICE } from "./dictionary-messages";

/** 命名词库一次导入的文本上限，与 client-services 的 `MAX_IMPORT_TEXT_BYTES` 相同。 */
const MAX_COLLECTION_IMPORT_BYTES = 16 * 1024 * 1024;

export interface DictionaryCollectionImportOptions {
  collections: Pick<DictionaryCollectionsClient, "importFile" | "importBytes">;
  source: Pick<CollectionImportSource, "format">;
  file: File;
  /** 由调用方在文件准备开始前捕获，读取期间失效时不再提交或返回提示。 */
  isCurrent: () => boolean;
  /** 沿用调用方的修改忙碌、错误提示、刷新及结果归属。 */
  run: (
    action: () => Promise<DictionaryCollectionsView>,
  ) => Promise<DictionaryCollectionsView | undefined>;
}

/** 读取并导入一个命名词库文件，返回当前操作的提示；不依赖设置表单或提示容器。 */
export async function importDictionaryCollectionFile({
  collections,
  source,
  file,
  isCurrent,
  run,
}: DictionaryCollectionImportOptions): Promise<string> {
  if (!isCurrent()) return "";
  const name = collectionNameFromFile(file.name);
  const format = collectionFormatForFile(file.name, source.format);
  if (binaryCollectionFormat(format)) {
    const importBytes = collections.importBytes;
    if (!importBytes) return collectionFailureMessage({ code: "unsupported_format" });
    let encoded: string;
    try {
      encoded = await readScelFile(file, MAX_SCEL_FILE_BYTES);
    } catch (error) {
      return isCurrent() ? exceptionMessage(error, "读取文件失败，请重新选择。") : "";
    }
    if (!isCurrent()) return "";
    const next = await run(() => importBytes.call(collections, name, format, encoded));
    return next && isCurrent()
      ? `${collectionImportMessage(name, next.import)}。${SCEL_PERSONAL_USE_NOTICE}`
      : "";
  }
  let text: string;
  try {
    text = await readDictionaryFile(file, MAX_COLLECTION_IMPORT_BYTES);
  } catch (error) {
    return isCurrent() ? exceptionMessage(error, "读取文件失败，请重新选择。") : "";
  }
  if (!isCurrent()) return "";
  const next = await run(() => collections.importFile(name, format, text));
  return next && isCurrent() ? collectionImportMessage(name, next.import) : "";
}
