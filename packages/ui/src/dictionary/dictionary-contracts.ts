import type {
  DictionaryEntry,
  LocalDictionaryFormat,
  LocalDictionaryKind,
  ScelText,
} from "./dictionary-file";

/** 一段文字（一个字或整段词）在两张五笔码表里的编码，各自短码在前；码表里没有时为空数组。 */
export interface WubiTextCodes {
  text: string;
  wubi86: string[];
  wubi98: string[];
}

/** 词库请求 `wubi_codes` 的回答（host-api `wubi_codes_json`）。 */
export interface WubiCodeLookup {
  /** 整段文字作为一个词的编码；整段不是 2 到 16 个汉字或两张表都没有它时为 null。 */
  phrase: WubiTextCodes | null;
  /** 文字里每个不同的汉字，按第一次出现的顺序。 */
  characters: WubiTextCodes[];
  /** 不同的汉字太多，后面的没有列出。 */
  truncated: boolean;
}

/** 宿主的查码入口：把文字交给词库请求 `wubi_codes`。 */
export type WubiCodeLookupClient = (text: string) => Promise<WubiCodeLookup>;

export type DictionaryFailure = { request_id: string; label: string; error: string };
/** 对应 `client-services::dictionary::import` 的导入响应。 */
export interface DictionaryImportResult {
  applied: number;
  /** 已检查但跳过的行数。 */
  failed: number;
  /** 超过单文件上限的行未被检查。 */
  truncated?: boolean;
  /** 文件两列与所选格式相反，已按文件本身的顺序读取。 */
  swapped?: boolean;
  first_failures?: { line: number; issue: string }[];
}
/** 随包词库的构建规格与来源提交。 */
export type DictionaryManifest = { profile: string; sourceCommit: string };

export interface DictionaryClient {
  /** ASCII 查询匹配编码前缀，含非 ASCII 字符时匹配词条字面内容；宿主在分页前完成筛选。 */
  list(
    offset: number,
    limit: number,
    kind?: LocalDictionaryKind,
    query?: string,
  ): Promise<{
    entries: DictionaryEntry[];
    has_more: boolean;
    pending_count?: number;
    failed_requests?: DictionaryFailure[];
    snapshot_error?: string | null;
    page_offset?: number;
    requested_page_offset?: number;
  }>;
  edit(
    previous: DictionaryEntry | null,
    replacement: DictionaryEntry | null,
    request_id: string,
  ): Promise<void>;
  import?(
    kind: LocalDictionaryKind,
    format: LocalDictionaryFormat,
    text: string,
    request_id: string,
  ): Promise<DictionaryImportResult>;
  /** 页面为 `import` 读取的文件字节上限。只有把一次导入整份作为一个请求发给宿主、不分批的客户端才设它（`UNBATCHED_DICTIONARY_FILE_BYTES`）；页面据此只送细胞词库转出文本的开头（`leadingImportRows`）。 */
  maxImportFileBytes?: number;
  /** 把搜狗细胞词库（`.scel` 的 base64）转成标准 TSV 文本；页面再以 `standard` 交给 `import`。宿主提供它时，拼音词库的文件格式里多一项细胞词库。 */
  scelText?(bytesBase64: string): Promise<ScelText>;
  importPersonal?(
    text: string,
    request_id: string,
  ): Promise<{ queued: boolean; pending_count: number }>;
  export?(
    kind: LocalDictionaryKind,
    format: Exclude<LocalDictionaryFormat, "rime" | "hans">,
    offset: number,
    limit: number,
  ): Promise<{ text: string; has_more: boolean }>;
  retry?(request_id: string): Promise<void>;
  dismissFailure?(request_id: string): Promise<void>;
  /** 一种词库的词条总数，含内置词条；手机「词库」页的「拼音词库」行显示它。 */
  count?(kind: LocalDictionaryKind): Promise<number>;
  /** 五笔查码（词库请求 `wubi_codes`，只读）：`text` 里每个汉字和整段词的 86、98 五笔编码。宿主提供它时，五笔的设置下面出现「五笔查码」。 */
  wubiCodes?(text: string): Promise<WubiCodeLookup>;
}
