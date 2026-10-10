import { SchemeTraits } from "../SchemeTraits";

/** 展开候选面板左栏放什么：数字串可能的拼音，或者五个笔画键。 */
export enum NineKeyPanelMode {
  SPELLING = "spelling",
  STROKE = "stroke",
}

/** 面板里的 ⌫ 这一下做什么：先撤掉一笔笔画筛选，没有笔画时才是普通的退格。 */
export enum NineKeyPanelBackspace {
  POP_STROKE = "pop_stroke",
  ENGINE = "engine",
}

/** 一个笔画键：交给 C ABI 的 ASCII 字母、显示的字形和读屏用的名字。 */
export interface NineKeyStroke {
  readonly code: string;
  readonly glyph: string;
  readonly name: string;
}

function stroke(code: string, glyph: string, name: string): NineKeyStroke {
  return { code: code, glyph: glyph, name: name };
}

// 与 `msime_client_set_nine_key_filter` 的字节一一对应：h 横、s 竖、p 撇、n 点、z 折；字形与笔画方案的读音行用的是同一套。
const STROKES: NineKeyStroke[] = [
  stroke("h", "一", "横"),
  stroke("s", "丨", "竖"),
  stroke("p", "丿", "撇"),
  stroke("n", "丶", "点"),
  stroke("z", "乛", "折"),
];

/**
 * 全拼九键展开候选面板（三栏：左拼音或笔画、中候选、右功能键）里不依赖界面的判断。
 *
 * 笔画前缀和「只留单字」的状态归引擎（`View.nine_key_strokes`、`View.nine_key_single_character`），这里只算下一次要发给 `msime_client_set_nine_key_filter` 的值，界面自己不另存一份。
 */
export class NineKeyPanelPolicy {
  /** 与 C ABI 的上限相同；更长的前缀不可能是任何字的笔顺。 */
  static readonly MAX_STROKES: number = 64;

  static strokes(): NineKeyStroke[] {
    return STROKES;
  }

  /**
   * 展开面板是否画成三栏：九键或 14 键键面（`gridFace`）上的全拼正在组字时，两种网格共用引擎的拼音栏和筛选。英文、本地模式（rulesScheme 为 -1）、日语和其他方案仍是原来的整块候选面板，它们没有拼音栏可放，也没有九键筛选可用。
   */
  static threeColumn(gridFace: boolean, rulesScheme: number, composing: boolean): boolean {
    return gridFace && rulesScheme === SchemeTraits.QUANPIN && composing;
  }

  /** 面板里的 ⌫：笔画模式下有笔画就先撤一笔，否则交给引擎退格（引擎在全部锁定时撤销最后一次锁定，否则删一个数字）。 */
  static backspace(mode: NineKeyPanelMode, strokes: string): NineKeyPanelBackspace {
    return mode === NineKeyPanelMode.STROKE && strokes.length > 0
      ? NineKeyPanelBackspace.POP_STROKE
      : NineKeyPanelBackspace.ENGINE;
  }

  /** 追加一笔后的前缀；不是五个笔画字母之一、或已到上限时原样返回，调用方据此不发请求。 */
  static appendStroke(strokes: string, code: string): string {
    if (!STROKES.some((item: NineKeyStroke): boolean => item.code === code)) {
      return strokes;
    }
    if (strokes.length >= NineKeyPanelPolicy.MAX_STROKES) {
      return strokes;
    }
    return strokes + code;
  }

  /** 撤掉最后一笔后的前缀。 */
  static popStroke(strokes: string): string {
    return strokes.length === 0 ? strokes : strokes.substring(0, strokes.length - 1);
  }

  /** 把笔画字母串画成字形，认不出的字节跳过。 */
  static glyphs(strokes: string): string {
    let text: string = "";
    for (let index: number = 0; index < strokes.length; index++) {
      const code: string = strokes.charAt(index);
      const found: NineKeyStroke | undefined = STROKES.find(
        (item: NineKeyStroke): boolean => item.code === code,
      );
      if (found !== undefined) {
        text += found.glyph;
      }
    }
    return text;
  }

  /** 切换拼音/笔画后要发给引擎的笔画前缀，null 表示不必发请求：离开笔画模式时清空已有的笔画，笔画筛选只属于笔画模式；进入笔画模式不改筛选。 */
  static strokesAfterToggle(mode: NineKeyPanelMode, strokes: string): string | null {
    return mode === NineKeyPanelMode.STROKE && strokes.length > 0 ? "" : null;
  }

  static toggledMode(mode: NineKeyPanelMode): NineKeyPanelMode {
    return mode === NineKeyPanelMode.STROKE ? NineKeyPanelMode.SPELLING : NineKeyPanelMode.STROKE;
  }

  /** 拼音/笔画键的键面：显示当前所在的模式。 */
  static modeTitle(mode: NineKeyPanelMode): string {
    return mode === NineKeyPanelMode.STROKE ? "笔画" : "拼音";
  }

  /** 读屏文字说的是按下去会切到哪一边。 */
  static modeLabel(mode: NineKeyPanelMode): string {
    return mode === NineKeyPanelMode.STROKE ? "切换到拼音" : "切换到笔画筛选";
  }

  /** 全部/单字键的键面：显示当前的筛选。 */
  static singleCharacterTitle(singleCharacter: boolean): string {
    return singleCharacter ? "单字" : "全部";
  }

  static singleCharacterLabel(singleCharacter: boolean): string {
    return singleCharacter ? "显示全部候选" : "只显示单字";
  }

  /** 关闭面板时是否要把筛选清掉：组字还在、且确实有筛选时才发请求，组字结束时引擎已经自己清了。 */
  static clearsFilterOnClose(
    composing: boolean,
    singleCharacter: boolean,
    strokes: string,
  ): boolean {
    return composing && (singleCharacter || strokes.length > 0);
  }
}
