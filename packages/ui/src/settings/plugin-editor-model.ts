import type {
  AuthoredKind,
  AuthoredPack,
  PluginCommand,
  PluginKind,
  PluginPhrase,
} from "./plugin-types";

/** 编辑器能写的类型，按「新建插件」里列出的顺序，每种带一句给不读文档的人看的说明。 */
export const AUTHORED_KINDS: readonly { kind: AuthoredKind; title: string; description: string }[] =
  [
    {
      kind: "phrase_table",
      title: "短语表",
      description: "在 K 模式里输入编码，打出一段常用文字，例如输入 dz 打出家庭地址。",
    },
    {
      kind: "command_table",
      title: "指令表",
      description: "按 / 再输入指令字母，插入签名、当天日期这类模板文字。",
    },
    {
      kind: "symbol_set",
      title: "符号集",
      description: "给符号面板加几组自己常用的符号或颜文字。",
    },
    {
      kind: "wordbook",
      title: "单词本",
      description: "做一本自己的词书，在背单词里复习。",
    },
  ];

/** 「新建插件」旁边的一句说明。 */
export const CREATE_DESCRIPTION =
  "不用写代码，填好短语、指令、符号或单词就能做出自己的插件，做好后也可以发布到社区";

/** 这个类型能不能在设置页编辑。 */
export function isAuthoredKind(kind: PluginKind): kind is AuthoredKind {
  return AUTHORED_KINDS.some((entry) => entry.kind === kind);
}

/** 新建插件时许可证的默认值。 */
export const DEFAULT_LICENSE = "CC0-1.0";
/** 新建插件时版本的默认值。 */
export const DEFAULT_VERSION = "1.0.0";

// 以下上限与 `crates/client-services/src/plugins/` 的清单校验一致，在这里先查一遍，让用户在提交之前就知道该改哪一行；宿主保存时按同一套规则再查。
const MAX_NAME_BYTES = 80;
const MAX_VERSION_BYTES = 32;
const MAX_LICENSE_BYTES = 64;
const MAX_AUTHOR_BYTES = 120;
const MAX_DESCRIPTION_BYTES = 500;
/** `phrase_table::MAX_PHRASES`。 */
export const MAX_PHRASES = 2000;
/** `command_table::MAX_COMMANDS`。 */
export const MAX_COMMANDS = 256;
const MAX_KEY_LETTERS = 32;
/** 短语文字和指令模板的 UTF-16 单元上限：Windows 候选管道的文本字段。 */
export const MAX_TEXT_UTF16 = 199;
/** 短语文字里光标标记的说明，与 Android、iOS、Windows 的短语表编辑器同一句；展开由共享层 `expand_quick_phrase` 做。 */
export const QUICK_PHRASE_CURSOR_NOTE =
  '在文字中用 ${cursor} 指定上屏后的光标位置，例如 title:"${cursor}" 会输入 title:""，光标停在引号内。只定位第一个标记；要输入标记本身，请写 $${cursor}。编辑器不支持定位时光标留在末尾。';
/** 词库页编辑用户快捷短语时的说明：同一套标记，另外提醒以前存过字面 ${cursor} 的短语要改写。 */
export const QUICK_PHRASE_DICTIONARY_CURSOR_NOTE = `${QUICK_PHRASE_CURSOR_NOTE}以前存过、要照原样输入 \${cursor} 的短语，请改写成 $\${cursor}。`;
const MAX_COMMAND_TITLE_BYTES = 48;
/** `symbol_set::MAX_GROUPS`。 */
export const MAX_GROUPS = 32;
const MAX_GROUP_ITEMS = 512;
const MAX_SYMBOL_ITEMS = 2048;
const MAX_GROUP_TITLE_BYTES = 48;
const MAX_KEYWORDS_BYTES = 256;
const MAX_ITEM_UTF16 = 64;
/** `vocabulary::wordbook::MAX_ENTRIES`。 */
export const MAX_WORDS = 20000;
const MAX_WORD_CHARS = 64;
const MAX_PHONETIC_CHARS = 64;
const MAX_GLOSS_CHARS = 256;
const MAX_WORDBOOK_NAME_CHARS = 64;

const KEY = /^[a-z]+$/;
const LICENSE = /^[A-Za-z0-9.\-+() ]+$/;
const CONTROL = /\p{Cc}/u;

const encoder = new TextEncoder();
const bytes = (value: string) => encoder.encode(value).length;
const characters = (value: string) => [...value].length;
const blank = (value: string) => value.trim().length === 0;

/** 编辑中的单词：音标留空时就是空串。 */
export type DraftWord = { word: string; phonetic: string; gloss: string };
/** 编辑中的一组符号：`items` 是文本框里的原文，一行一项。 */
export type DraftGroup = {
  tab: "symbols" | "kaomoji";
  title: string;
  keywords: string;
  items: string;
};

/** 编辑器的草稿：全部字段都是表单里的原文，保存前由 `packFromDraft` 整理。四种内容都在，只有与 `kind` 对应的那一种会保存。 */
export type AuthoringDraft = {
  kind: AuthoredKind;
  /** 编辑已安装的包时是它的 id；新建时为 null，由宿主生成。 */
  id: string | null;
  name: string;
  version: string;
  license: string;
  author: string;
  description: string;
  phrases: PluginPhrase[];
  commands: PluginCommand[];
  groups: DraftGroup[];
  words: DraftWord[];
};

export const emptyPhrase = (): PluginPhrase => ({ key: "", text: "" });
export const emptyCommand = (): PluginCommand => ({ trigger: "", title: "", template: "" });
export const emptyGroup = (): DraftGroup => ({
  tab: "symbols",
  title: "",
  keywords: "",
  items: "",
});
export const emptyWord = (): DraftWord => ({ word: "", phonetic: "", gloss: "" });

/** 新建一种插件时的草稿：默认许可证和版本，内容先给一个空行。 */
export function emptyDraft(kind: AuthoredKind): AuthoringDraft {
  return {
    kind,
    id: null,
    name: "",
    version: DEFAULT_VERSION,
    license: DEFAULT_LICENSE,
    author: "",
    description: "",
    phrases: kind === "phrase_table" ? [emptyPhrase()] : [],
    commands: kind === "command_table" ? [emptyCommand()] : [],
    groups: kind === "symbol_set" ? [emptyGroup()] : [],
    words: kind === "wordbook" ? [emptyWord()] : [],
  };
}

/** 宿主读出的已安装包，变成可以编辑的草稿。 */
export function draftFromPack(pack: AuthoredPack): AuthoringDraft {
  return {
    kind: pack.kind,
    id: pack.id,
    name: pack.name,
    version: pack.version,
    license: pack.license,
    author: pack.author ?? "",
    description: pack.description ?? "",
    phrases: (pack.phrases ?? []).map((row) => ({ key: row.key, text: row.text })),
    commands: (pack.commands ?? []).map((row) => ({
      trigger: row.trigger,
      title: row.title,
      template: row.template,
    })),
    groups: (pack.groups ?? []).map((group) => ({
      tab: group.tab,
      title: group.title,
      keywords: group.keywords ?? "",
      items: group.items.join("\n"),
    })),
    words: (pack.words ?? []).map((entry) => ({
      word: entry.word,
      phonetic: entry.phonetic ?? "",
      gloss: entry.gloss,
    })),
  };
}

const isBlankPhrase = (row: PluginPhrase) => blank(row.key) && blank(row.text);
const isBlankCommand = (row: PluginCommand) =>
  blank(row.trigger) && blank(row.title) && blank(row.template);
const isBlankGroup = (group: DraftGroup) =>
  blank(group.title) && blank(group.keywords) && blank(group.items);
const isBlankWord = (row: DraftWord) => blank(row.word) && blank(row.phonetic) && blank(row.gloss);

/** 文本框里的符号：一行一项，去掉首尾空白，跳过空行。 */
export function groupItems(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

const optional = (value: string) => (blank(value) ? null : value.trim());

/** 草稿整理成交给宿主的包：各字段去掉首尾空白，可选字段留空时为 null，整行都空着的行不保存。 */
export function packFromDraft(draft: AuthoringDraft): AuthoredPack {
  const pack: AuthoredPack = {
    kind: draft.kind,
    id: draft.id,
    name: draft.name.trim(),
    version: draft.version.trim(),
    license: draft.license.trim(),
    author: optional(draft.author),
    description: optional(draft.description),
  };
  switch (draft.kind) {
    case "phrase_table":
      pack.phrases = draft.phrases
        .filter((row) => !isBlankPhrase(row))
        .map((row) => ({ key: row.key.trim(), text: row.text.trim() }));
      break;
    case "command_table":
      pack.commands = draft.commands
        .filter((row) => !isBlankCommand(row))
        .map((row) => ({
          trigger: row.trigger.trim(),
          title: row.title.trim(),
          template: row.template.trim(),
        }));
      break;
    case "symbol_set":
      pack.groups = draft.groups
        .filter((group) => !isBlankGroup(group))
        .map((group) => ({
          tab: group.tab,
          title: group.title.trim(),
          keywords: optional(group.keywords),
          items: groupItems(group.items),
        }));
      break;
    case "wordbook":
      pack.words = draft.words
        .filter((row) => !isBlankWord(row))
        .map((row) => ({
          word: row.word.trim(),
          phonetic: optional(row.phonetic),
          gloss: row.gloss.trim(),
        }));
      break;
  }
  return pack;
}

/** 指令模板里的占位符是否都认得：`{date}`、`{time}`（都可以带 `:格式`）和 `{weekday}`，花括号成对且不嵌套。格式本身对不对、展开后会不会太长由宿主检查。 */
function templatePlaceholdersKnown(template: string): boolean {
  let rest = template;
  for (;;) {
    const open = rest.search(/[{}]/);
    if (open < 0) return true;
    if (rest[open] === "}") return false;
    const after = rest.slice(open + 1);
    const close = after.search(/[{}]/);
    if (close < 0 || after[close] === "{") return false;
    const placeholder = after.slice(0, close);
    if (placeholder !== "weekday" && !/^(?:date|time)(?::.*)?$/s.test(placeholder)) return false;
    rest = after.slice(close + 1);
  }
}

/** 草稿现在不能保存的第一个原因，用页面上的说法；能保存时为 null。规则与宿主的清单校验一致，行号按表单里的行数（整行都空着的行不算问题）。 */
export function draftIssue(draft: AuthoringDraft): string | null {
  const name = draft.name.trim();
  if (!name) return "请给插件起个名字。";
  if (bytes(name) > MAX_NAME_BYTES || CONTROL.test(name)) return "名字太长了。";
  if (draft.kind === "wordbook" && characters(name) > MAX_WORDBOOK_NAME_CHARS)
    return `单词本的名字不能超过 ${MAX_WORDBOOK_NAME_CHARS} 个字。`;
  const version = draft.version.trim();
  if (!version) return "请填写版本，例如 1.0.0。";
  if (bytes(version) > MAX_VERSION_BYTES || CONTROL.test(version)) return "版本太长了。";
  const license = draft.license.trim();
  if (!license) return `请填写许可证，不确定时写 ${DEFAULT_LICENSE}。`;
  if (bytes(license) > MAX_LICENSE_BYTES || !LICENSE.test(license))
    return "许可证只能写 SPDX 许可证代码，例如 CC0-1.0、CC-BY-4.0。";
  if (bytes(draft.author.trim()) > MAX_AUTHOR_BYTES || CONTROL.test(draft.author))
    return "作者太长了。";
  if (bytes(draft.description.trim()) > MAX_DESCRIPTION_BYTES || CONTROL.test(draft.description))
    return "介绍太长了。";
  switch (draft.kind) {
    case "phrase_table":
      return phrasesIssue(draft.phrases);
    case "command_table":
      return commandsIssue(draft.commands);
    case "symbol_set":
      return groupsIssue(draft.groups);
    case "wordbook":
      return wordsIssue(draft.words);
  }
}

function phrasesIssue(rows: readonly PluginPhrase[]): string | null {
  const seen = new Set<string>();
  let count = 0;
  for (const [index, row] of rows.entries()) {
    if (isBlankPhrase(row)) continue;
    count++;
    const at = `第 ${index + 1} 条短语`;
    const key = row.key.trim();
    const text = row.text.trim();
    if (!key) return `${at}还没有填写编码。`;
    if (key.length > MAX_KEY_LETTERS || !KEY.test(key))
      return `${at}的编码只能是 1 到 ${MAX_KEY_LETTERS} 个小写字母。`;
    if (!text) return `${at}还没有填写文字。`;
    if (text.length > MAX_TEXT_UTF16) return `${at}的文字太长了。`;
    if (CONTROL.test(text)) return `${at}的文字里不能有换行或制表符。`;
    const pair = `${key}\n${text}`;
    if (seen.has(pair)) return `${at}和前面的一条重复了。`;
    seen.add(pair);
  }
  if (count === 0) return "至少要有一条短语。";
  if (count > MAX_PHRASES) return `一个短语表最多 ${MAX_PHRASES} 条。`;
  return null;
}

function commandsIssue(rows: readonly PluginCommand[]): string | null {
  const triggers = new Set<string>();
  let count = 0;
  for (const [index, row] of rows.entries()) {
    if (isBlankCommand(row)) continue;
    count++;
    const at = `第 ${index + 1} 条指令`;
    const trigger = row.trigger.trim();
    const title = row.title.trim();
    const template = row.template.trim();
    if (!trigger) return `${at}还没有填写指令字母。`;
    if (trigger.length > MAX_KEY_LETTERS || !KEY.test(trigger))
      return `${at}的指令只能是 1 到 ${MAX_KEY_LETTERS} 个小写字母。`;
    if (triggers.has(trigger)) return `指令 /${trigger} 重复了。`;
    triggers.add(trigger);
    if (!title) return `${at}还没有填写标题。`;
    if (bytes(title) > MAX_COMMAND_TITLE_BYTES || CONTROL.test(title)) return `${at}的标题太长了。`;
    if (!template) return `${at}还没有填写要插入的文字。`;
    if (template.length > MAX_TEXT_UTF16) return `${at}要插入的文字太长了。`;
    if (CONTROL.test(template)) return `${at}要插入的文字里不能有换行或制表符。`;
    if (!templatePlaceholdersKnown(template))
      return `${at}的花括号里只能写 {date}、{time} 或 {weekday}。`;
  }
  if (count === 0) return "至少要有一条指令。";
  if (count > MAX_COMMANDS) return `一个指令表最多 ${MAX_COMMANDS} 条。`;
  return null;
}

function groupsIssue(groups: readonly DraftGroup[]): string | null {
  const titles = new Set<string>();
  let count = 0;
  let total = 0;
  for (const [index, group] of groups.entries()) {
    if (isBlankGroup(group)) continue;
    count++;
    const at = `第 ${index + 1} 组`;
    const title = group.title.trim();
    if (!title) return `${at}还没有填写标题。`;
    if (bytes(title) > MAX_GROUP_TITLE_BYTES || CONTROL.test(title)) return `${at}的标题太长了。`;
    const titleKey = `${group.tab}\n${title}`;
    if (titles.has(titleKey)) return `${at}的标题和同一页的另一组重复了。`;
    titles.add(titleKey);
    if (bytes(group.keywords.trim()) > MAX_KEYWORDS_BYTES || CONTROL.test(group.keywords))
      return `${at}的搜索关键词太长了。`;
    const items = groupItems(group.items);
    if (items.length === 0) return `${at}还没有符号。`;
    if (items.length > MAX_GROUP_ITEMS) return `${at}最多 ${MAX_GROUP_ITEMS} 项。`;
    total += items.length;
    const seen = new Set<string>();
    for (const item of items) {
      if (item.length > MAX_ITEM_UTF16 || CONTROL.test(item)) return `${at}的「${item}」太长了。`;
      if (seen.has(item)) return `${at}里「${item}」重复了。`;
      seen.add(item);
    }
  }
  if (count === 0) return "至少要有一组符号。";
  if (count > MAX_GROUPS) return `一个符号集最多 ${MAX_GROUPS} 组。`;
  if (total > MAX_SYMBOL_ITEMS) return `一个符号集合计最多 ${MAX_SYMBOL_ITEMS} 项。`;
  return null;
}

function wordsIssue(rows: readonly DraftWord[]): string | null {
  const seen = new Set<string>();
  let count = 0;
  for (const [index, row] of rows.entries()) {
    if (isBlankWord(row)) continue;
    count++;
    const at = `第 ${index + 1} 个单词`;
    const word = row.word.trim();
    const phonetic = row.phonetic.trim();
    const gloss = row.gloss.trim();
    if (!word) return `${at}还没有填写单词。`;
    if (characters(word) > MAX_WORD_CHARS || CONTROL.test(word)) return `${at}太长了。`;
    if (word.startsWith("#")) return `${at}不能以 # 开头。`;
    if (characters(phonetic) > MAX_PHONETIC_CHARS || CONTROL.test(phonetic))
      return `${at}的音标太长了。`;
    if (!gloss) return `${at}还没有填写释义。`;
    if (characters(gloss) > MAX_GLOSS_CHARS || CONTROL.test(gloss)) return `${at}的释义太长了。`;
    if (seen.has(word)) return `「${word}」出现了不止一次。`;
    seen.add(word);
  }
  if (count === 0) return "至少要有一个单词。";
  if (count > MAX_WORDS) return `一本单词本最多 ${MAX_WORDS} 个单词。`;
  return null;
}

/** 批量粘贴的结果：能读出的行，以及第一行读不出的原因。 */
export type BulkRows<T> = { rows: T[]; issue: string | null };

/** 批量添加短语：每行「编码 文字」，编码和文字之间用空格或 Tab 隔开，文字里可以再有空格；空行跳过。 */
export function parsePhraseLines(text: string): BulkRows<PluginPhrase> {
  const rows: PluginPhrase[] = [];
  for (const [index, line] of text.split(/\r?\n/).entries()) {
    if (blank(line)) continue;
    const match = /^\s*(\S+)\s+(.+?)\s*$/.exec(line);
    if (!match) return { rows, issue: `第 ${index + 1} 行要写成「编码 文字」，中间用空格隔开。` };
    rows.push({ key: match[1], text: match[2] });
  }
  return { rows, issue: null };
}

/** 批量添加单词：每行「单词 Tab 释义」或「单词 Tab 音标 Tab 释义」，从表格复制过来就是这样；空行和 # 开头的行跳过，与单词本的词表文件同一种写法。 */
export function parseWordLines(text: string): BulkRows<DraftWord> {
  const rows: DraftWord[] = [];
  for (const [index, line] of text.split(/\r?\n/).entries()) {
    if (blank(line) || line.startsWith("#")) continue;
    const columns = line.split("\t");
    if (columns.length === 2) rows.push({ word: columns[0], phonetic: "", gloss: columns[1] });
    else if (columns.length === 3)
      rows.push({ word: columns[0], phonetic: columns[1], gloss: columns[2] });
    else
      return {
        rows,
        issue: `第 ${index + 1} 行要写成「单词、释义」两列或「单词、音标、释义」三列，列之间用 Tab 隔开。`,
      };
  }
  return { rows, issue: null };
}

/** 把批量读出的行接到已有的行后面；已有的只剩一个空行时直接替换它。 */
export function appendRows<T>(
  current: readonly T[],
  added: readonly T[],
  isBlank: (row: T) => boolean,
): T[] {
  const kept = current.length === 1 && isBlank(current[0]) ? [] : current;
  return [...kept, ...added];
}

export const blankRow = {
  phrase: isBlankPhrase,
  word: isBlankWord,
};

/** 刚保存的包详情页顶上的一句话：短语表和指令表还要打开「启用」，K 模式或 / 指令关着时也要打开，别的类型装上就能用。`enabled` 是这个表已经启用，`modeOn` 是对应的快捷模式开着。 */
export function authoredSaveNote(kind: PluginKind, enabled: boolean, modeOn: boolean): string {
  switch (kind) {
    case "phrase_table":
      if (enabled)
        return modeOn
          ? "已保存，修改已经生效。"
          : "已保存。快捷短语（K 模式）现在关着，在「输入 → 快捷模式」打开后才会出候选。";
      return modeOn
        ? "已保存。打开下面的「启用」后，按 Shift+K 再输入编码就能用到这些短语。"
        : "已保存。打开下面的「启用」，再在「输入 → 快捷模式」打开快捷短语（K 模式），这些短语才会出候选。";
    case "command_table":
      if (enabled)
        return modeOn
          ? "已保存，修改已经生效。"
          : "已保存。/ 指令现在关着，在「输入 → 快捷模式」打开后才能用。";
      return modeOn
        ? "已保存。打开下面的「启用」后，按 / 再输入指令字母就能用。"
        : "已保存。打开下面的「启用」，再在「输入 → 快捷模式」打开 / 指令，这些指令才能用。";
    case "symbol_set":
      return "已保存，符号面板里会显示这些符号。";
    case "wordbook":
      return "已保存，这本书出现在背单词的词书里。";
    default:
      return "已保存。";
  }
}
