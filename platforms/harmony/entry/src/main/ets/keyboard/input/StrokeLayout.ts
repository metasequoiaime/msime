/**
 * 笔画方案的触屏键位：九键外框中间的 2×3 网格，每个键是一种笔画或通配符。
 *
 * 引擎按小写字母读笔画（crates/engine/src/stroke/mod.rs）：h 横、s 竖、p 撇、n 点、z 折，x 匹配任意一笔。键面画笔画字形，与引擎在预编辑里画的字形相同；点击发送对应字母，组字、候选都归引擎。乛 选 U+4E5B 而不是 CJK 笔画区或扩展 B 的折笔字形，免得系统字体缺字。
 */

/** 一个笔画键：发给引擎的字母、键面字形、字形下方的小字、读屏念出的名字。 */
export interface StrokeKey {
  readonly input: string;
  readonly face: string;
  readonly hint: string;
  readonly label: string;
}

/** 通配键的字母：组字中追加一个匹配任意一笔的位置，空组合时引擎不接它。 */
const WILDCARD: string = "x";

const ROWS: StrokeKey[][] = [
  [
    { input: "h", face: "一", hint: "横", label: "横" },
    { input: "s", face: "丨", hint: "竖", label: "竖" },
    { input: "p", face: "丿", hint: "撇", label: "撇" },
  ],
  [
    { input: "n", face: "丶", hint: "点", label: "点" },
    { input: "z", face: "乛", hint: "折", label: "折" },
    { input: WILDCARD, face: "＊", hint: "通配", label: "通配符" },
  ],
];

export class StrokeLayout {
  static readonly ROWS: StrokeKey[][] = ROWS;
  static readonly WILDCARD: string = WILDCARD;

  /** 五种笔画之一，也就是空组合时能开始组字的键。 */
  static startsComposition(input: string): boolean {
    return input !== WILDCARD && StrokeLayout.find(input) !== undefined;
  }

  /**
   * 点了这个键是否要发给引擎。空组合时引擎不接通配键（按 msime_client.h 的约定交回宿主插入），而触屏上把一个 x 插进文档不是用户想要的，所以通配键只在组字中才发送。
   */
  static sends(input: string, composing: boolean): boolean {
    return composing
      ? StrokeLayout.find(input) !== undefined
      : StrokeLayout.startsComposition(input);
  }

  private static find(input: string): StrokeKey | undefined {
    for (const row of ROWS) {
      for (const key of row) {
        if (key.input === input) {
          return key;
        }
      }
    }
    return undefined;
  }
}
