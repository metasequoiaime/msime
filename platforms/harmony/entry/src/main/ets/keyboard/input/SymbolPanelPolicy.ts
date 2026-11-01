import { KeyboardGeometry } from "../KeyboardGeometry";
import { EMAIL_SUFFIXES } from "./EmailSuffixPolicy";
import { PairedPunctuationPolicy } from "./PairedPunctuationPolicy";

/** The independent symbol keyboard, separate from the Unicode emoji catalog. */
export interface SymbolPanelCategory {
  readonly id: string;
  readonly title: string;
  readonly symbols: string[];
}

export interface SymbolPanelSelection {
  readonly locked: boolean;
  readonly returnToKeyboard: boolean;
}

const CATEGORIES: SymbolPanelCategory[] = [
  {
    id: "common",
    title: "常用",
    symbols: [
      "，",
      "。",
      "？",
      "！",
      "、",
      "；",
      "：",
      "…",
      "—",
      "·",
      "“",
      "”",
      "‘",
      "’",
      "～",
      "（",
      "）",
      "《",
      "》",
      "￥",
      "【",
      "】",
      "＆",
      "＃",
      "＠",
      "％",
      "＋",
      "－",
      "＝",
      "／",
    ],
  },
  {
    id: "chinese",
    title: "中文",
    // 中文标点和全角符号排在最前，与 Android、iOS 的「中文」同一份同一顺序。每行五格，成对的括号和引号两两占左边四格、右边一格放单个符号，前后两半不会分在两行（`SymbolPanelPolicy.cells`）。
    symbols: [
      "，",
      "。",
      "？",
      "！",
      "、",
      "；",
      "：",
      "…",
      "—",
      "·",
      "“",
      "”",
      "‘",
      "’",
      "～",
      "（",
      "）",
      "《",
      "》",
      "＆",
      "【",
      "】",
      "＃",
      "＠",
      "％",
      "＋",
      "－",
      "＝",
      "／",
      "￥",
      "〈",
      "〉",
      "「",
      "」",
      "〇",
      "『",
      "』",
      "〔",
      "〕",
      "※",
      "〖",
      "〗",
      "＜",
      "＞",
      "°",
      "｛",
      "｝",
      "［",
      "］",
      "℃",
      "︵",
      "︶",
      "﹁",
      "﹂",
      "±",
      "×",
      "÷",
      "≈",
      "≠",
      "≤",
      "≥",
      "√",
      "∞",
      "∵",
      "∴",
      "→",
      "←",
      "↑",
      "↓",
      "★",
      "☆",
      "●",
      "○",
      "■",
      "□",
      "◆",
      "◇",
      "▲",
      "△",
    ],
  },
  {
    id: "english",
    title: "英文",
    symbols: [
      ",",
      ".",
      "?",
      "!",
      ";",
      ":",
      "'",
      '"',
      "(",
      ")",
      "[",
      "]",
      "{",
      "}",
      "/",
      "<",
      ">",
      "\\",
      "|",
      "-",
      "_",
      "+",
      "=",
      "*",
      "&",
      "^",
      "%",
      "$",
      "#",
      "@",
      "~",
      "`",
      "·",
      "…",
      "–",
      "—",
      "§",
      "¶",
      "†",
      "‡",
    ],
  },
  {
    id: "number",
    title: "数字",
    symbols: [
      "0",
      "1",
      "2",
      "3",
      "4",
      "5",
      "6",
      "7",
      "8",
      "9",
      "①",
      "②",
      "③",
      "④",
      "⑤",
      "⑥",
      "⑦",
      "⑧",
      "⑨",
      "⑩",
      "一",
      "二",
      "三",
      "四",
      "五",
      "六",
      "七",
      "八",
      "九",
      "十",
      "Ⅰ",
      "Ⅱ",
      "Ⅲ",
      "Ⅳ",
      "Ⅴ",
      "Ⅵ",
      "Ⅶ",
      "Ⅷ",
      "Ⅸ",
      "Ⅹ",
      "½",
      "⅓",
      "¼",
      "‰",
      "′",
      "″",
      "㎡",
      "㎏",
      "㎝",
      "№",
    ],
  },
  {
    id: "network",
    title: "网络",
    symbols: [
      "@",
      "#",
      "/",
      "\\",
      ":",
      "_",
      "-",
      "+",
      "=",
      "&",
      "?",
      "%",
      "~",
      "^",
      "*",
      "|",
      "<",
      ">",
      "$",
      "€",
      "http://",
      "https://",
      "www.",
      ".com",
      ".cn",
      ".net",
      ".org",
      ".io",
      // 网址片段之后接全部常用邮箱后缀，与 Android、iOS 的「网络」和候选栏给的后缀是同一份。
      ...EMAIL_SUFFIXES,
    ],
  },
];

const MAX_SYMBOLS_PER_CATEGORY: number = 96;

export class SymbolPanelPolicy {
  static categories(): SymbolPanelCategory[] {
    return CATEGORIES.map((category: SymbolPanelCategory): SymbolPanelCategory => ({
      id: category.id,
      title: category.title,
      symbols: category.symbols.slice(0, MAX_SYMBOLS_PER_CATEGORY),
    }));
  }

  static category(index: number): SymbolPanelCategory {
    const categories: SymbolPanelCategory[] = SymbolPanelPolicy.categories();
    const safeIndex: number = Number.isFinite(index)
      ? KeyboardGeometry.bounded(Math.floor(index), 0, categories.length - 1)
      : 0;
    return categories[safeIndex];
  }

  /**
   * 网格里从左到右、从上到下的格子，`""` 是留空的一格。成对符号的前半个落在行尾、后半个紧跟在后时，这一格留空，两半一起排到下一行，（ 和 ） 不分在两行，与 Android `SymbolPanelModel.breaksBeforePair` 相同；成对与否按 `PairedPunctuationPolicy.literalPairClosing`，与 Android 是同一张表。写死的分类排好了顺序，不会留空。
   */
  static cells(symbols: string[], columns: number): string[] {
    const cells: string[] = [];
    for (let index = 0; index < symbols.length; index++) {
      const symbol: string = symbols[index];
      if (
        cells.length % columns === columns - 1 &&
        index + 1 < symbols.length &&
        PairedPunctuationPolicy.literalPairClosing(symbol + symbols[index + 1]) !== null
      ) {
        cells.push("");
      }
      cells.push(symbol);
    }
    return cells;
  }

  static select(locked: boolean): SymbolPanelSelection {
    return { locked: locked, returnToKeyboard: !locked };
  }

  static isValidSymbol(symbol: string): boolean {
    return (
      typeof symbol === "string" &&
      symbol.length > 0 &&
      symbol.length <= 16 &&
      !/[\u0000-\u001f\u007f]/.test(symbol)
    );
  }

  static maxSymbolsPerCategory(): number {
    return MAX_SYMBOLS_PER_CATEGORY;
  }
}
