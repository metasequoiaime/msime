import type { CommunityResource } from "../community/community-resources";
import { formatZhNumber } from "../core/format-number";

/**
 * 命名词库（`msime_client_dictionary_collections`）在界面上的形状，与 client-core `dictionary/collections.rs` 的 `DictionaryCollectionsView` 一致。
 *
 * Engine 只有一个用户词库；命名词库是它上面的一层元数据，词条经个人词库队列分批送进 Engine，所以启用、停用、删除只增删「不属于别的已启用词库、也不是用户自己加的」那些词。内置拼音词库不是集合，任何修改都会得到 `builtin_locked`。
 */
export type DictionaryCollectionKind = "pinyin" | "wubi" | "quickPhrase" | "english";

export type DictionaryCollectionSource =
  | { type: "user" }
  | { type: "import"; format: string }
  | { type: "community"; resource_id: string; revision: number };

export interface DictionaryCollection {
  id: string;
  name: string;
  kind: DictionaryCollectionKind;
  source: DictionaryCollectionSource;
  enabled: boolean;
  entry_count: number;
  /** 还没交给个人词库队列的增删条数。 */
  pending: number;
}

export interface DictionaryCollectionWord {
  kind: DictionaryCollectionKind;
  key: string;
  value: string;
  weight: number;
}

export interface DictionaryCollectionImportReport {
  imported: number;
  duplicates: number;
  failed: number;
  truncated: boolean;
  swapped: boolean;
}

export interface DictionaryCollectionsView {
  collections: DictionaryCollection[];
  /** client-core 接受的导入格式。 */
  formats: string[];
  import?: DictionaryCollectionImportReport;
  sent?: number;
}

/** 宿主提供的命名词库操作；每个操作都返回整份视图，失败时抛出带 `code` 的记录。 */
export interface DictionaryCollectionsClient {
  load(): Promise<DictionaryCollectionsView>;
  /** 把还没送出的增删再送一批，「刷新词库」用。 */
  flush(): Promise<DictionaryCollectionsView>;
  create(name: string): Promise<DictionaryCollectionsView>;
  delete(id: string): Promise<DictionaryCollectionsView>;
  setEnabled(id: string, enabled: boolean): Promise<DictionaryCollectionsView>;
  addWords(id: string, entries: DictionaryCollectionWord[]): Promise<DictionaryCollectionsView>;
  /** 导入成一个新词库。 */
  importFile(name: string, format: string, text: string): Promise<DictionaryCollectionsView>;
  /** 把社区词库装成一个词库；再装同一个词库会更新它，而不是再建一个。 */
  installCommunity(resource: CommunityResource): Promise<DictionaryCollectionsView>;
}

export const MAX_COLLECTION_NAME_CHARS = 32;

/** 词库名能否使用：1–32 个字，首尾没有空白，没有控制字符。与 client-core 的校验相同，提交前先在界面上挡住。 */
export function validCollectionName(name: string): boolean {
  if (name.length === 0 || name !== name.trim()) return false;
  if (Array.from(name).length > MAX_COLLECTION_NAME_CHARS) return false;
  return !/\p{Cc}/u.test(name);
}

/** 拼音编码能否作为新词的编码：小写字母，音节之间可以用撇号分隔，不以撇号开头或结尾。 */
export function validPinyinCode(code: string): boolean {
  return code.length > 0 && code.length <= 64 && /^[a-z]+(?:'[a-z]+)*$/.test(code);
}

/** 把用户输入的拼音收成编码：去掉首尾空白、转小写，空格和中文撇号都当作音节分隔。 */
export function normalizePinyinCode(input: string): string {
  return input
    .trim()
    .toLowerCase()
    .replace(/[‘’]/g, "'")
    .replace(/\s+/g, "'");
}

/** 编码的展示写法：音节之间用排版撇号（`pin’yin`）。 */
export function displayPinyinCode(code: string): string {
  return code.replace(/'/g, "’");
}

/** 从文件名得到新词库的名字：去掉扩展名（`.dict.yaml` 算一个）和控制字符，截到 32 个字，收不出来时用「导入的词库」。 */
export function collectionNameFromFile(fileName: string): string {
  let name = fileName.trim();
  if (name.toLowerCase().endsWith(".dict.yaml")) {
    name = name.slice(0, -".dict.yaml".length);
  } else {
    const dot = name.lastIndexOf(".");
    if (dot >= 0) name = name.slice(0, dot);
  }
  const kept = Array.from(name.replace(/\p{Cc}/gu, ""))
    .slice(0, MAX_COLLECTION_NAME_CHARS)
    .join("")
    .trim();
  return validCollectionName(kept) ? kept : "导入的词库";
}

/** 按文件名推断格式：`.yaml`、`.yml` 是 Rime 词典，其余用来源列表里选的格式。 */
export function collectionFormatForFile(fileName: string, chosen: string): string {
  const lower = fileName.toLowerCase();
  return lower.endsWith(".yaml") || lower.endsWith(".yml") ? "rime" : chosen;
}

export interface CollectionImportSource {
  format: string;
  title: string;
  accept: string;
}

/** 「导入词库」的来源：只列 client-core 接受的格式。`standard` 和 `windows` 是文本文件的两种列顺序，client-core 在 `txt` 里自动识别，不单独列出。与 Android 的 `importSources` 相同。 */
export function collectionImportSources(formats: readonly string[]): CollectionImportSource[] {
  const sources: CollectionImportSource[] = [];
  if (formats.includes("txt")) {
    sources.push({ format: "txt", title: "文本文件（.txt）", accept: ".txt,text/plain" });
  }
  if (formats.includes("rime")) {
    sources.push({
      format: "rime",
      title: "Rime 词典（.dict.yaml）",
      accept: ".yaml,.yml,text/plain",
    });
  }
  if (formats.includes("hans")) {
    sources.push({ format: "hans", title: "纯汉字词表（每行一个词）", accept: ".txt,text/plain" });
  }
  return sources;
}

/** 导入完成后的提示，与 Android 的写法一致。 */
export function collectionImportMessage(
  name: string,
  report: DictionaryCollectionImportReport | undefined,
): string {
  if (!report) return `已导入「${name}」`;
  let message = `已导入「${name}」，${report.imported} 条`;
  if (report.duplicates > 0) message += `，跳过重复 ${report.duplicates} 条`;
  if (report.failed > 0) message += `，${report.failed} 行无法识别`;
  if (report.truncated) message += "，词库已满";
  return message;
}

/** client-core 的错误码换成可直接展示的话；不认识的码按通用失败处理。与 Android 的 `failureMessage` 相同。 */
export function collectionFailureMessage(error: unknown): string {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : "";
  switch (code) {
    case "collections_name_invalid":
      return "词库名需要 1–32 个字，首尾不能有空格。";
    case "collections_limit":
      return "词库数量或词条数已达上限。";
    case "collections_not_found":
      return "这个词库已经不在了，列表已刷新。";
    case "builtin_locked":
      return "内置词库始终启用，不能停用或改名。";
    case "unsupported_format":
      return "暂不支持这种文件格式。";
    case "import_empty":
      return "文件是空的。";
    case "collections_too_large":
      return "文件太大，词库存不下。";
    case "import_control_characters":
      return "文件里有无法识别的字符，请确认是文本格式。";
    case "import_no_usable_rows":
      return "没有读到可用的词条，请检查文件格式。";
    case "collections_corrupt":
      return "词库文件已损坏，为避免丢失没有改动它。";
    case "personal_dictionary_busy":
      return "键盘正在整理词库，请稍后再试。";
    default:
      return "词库操作失败，请稍后重试。";
  }
}

/** 已安装列表里一个词库的副标题：「1,234 条」，社区来的加「 · 社区」，还有没送出的增删时加「 · n 条待同步」。 */
export function collectionSubtitle(collection: DictionaryCollection): string {
  const parts = [`${formatZhNumber(collection.entry_count)} 条`];
  if (collection.source.type === "community") parts.push("社区");
  if (collection.pending > 0) parts.push(`${formatZhNumber(collection.pending)} 条待同步`);
  return parts.join(" · ");
}
