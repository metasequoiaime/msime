import { KeyboardGeometry } from '../KeyboardGeometry';

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
      "（",
      "）",
      "《",
      "》",
      "【",
      "】",
      "～",
      "￥",
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
    symbols: [
      "〈",
      "〉",
      "「",
      "」",
      "『",
      "』",
      "〔",
      "〕",
      "〖",
      "〗",
      "＜",
      "＞",
      "｛",
      "｝",
      "［",
      "］",
      "︵",
      "︶",
      "﹁",
      "﹂",
      "￥",
      "〇",
      "※",
      "°",
      "℃",
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
      "<",
      ">",
      "/",
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
      "@qq.com",
      "@gmail.com",
    ],
  },
];

const MAX_SYMBOLS_PER_CATEGORY: number = 64;

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
