/**
 * 全拼 14 键的键表：QWERTY 上相邻两个字母合成一个键（L、M 单独一键），与 Android 的 `FourteenKeyLayout.java`、iOS 的 `FourteenKeyLayout.swift` 是同一张表。
 *
 * 点一下送的是这一组，不是某个字母：宿主把组的首字母交给 `msime_client_grid_key`，引擎归成组码并按网格解码。键表只是数据，怎么解码是引擎的事。
 */
export interface FourteenKey {
  /** 这一键上的小写字母，按键面从左到右。 */
  readonly letters: string;
  /** 交给引擎的字母：这一组的首字母，也就是组码。 */
  readonly input: string;
  /** 键面：两个大写字母，没有角标。 */
  readonly label: string;
  /** 读屏说的话：「按键 Q W」，单字母键是「字母 L」。 */
  readonly description: string;
}

function key(letters: string): FourteenKey {
  const upper: string = letters.toUpperCase();
  return {
    letters: letters,
    input: letters.charAt(0),
    label: upper,
    description: letters.length === 1 ? `字母 ${upper}` : `按键 ${upper.split("").join(" ")}`,
  };
}

// 三行字母，第二行不缩进；第三行两端的分词键和删除键由视图画，不在表里。
const ROWS: FourteenKey[][] = [
  [key("qw"), key("er"), key("ty"), key("ui"), key("op")],
  [key("as"), key("df"), key("gh"), key("jk"), key("l")],
  [key("zx"), key("cv"), key("bn"), key("m")],
];

export class FourteenKeyLayout {
  static rows(): FourteenKey[][] {
    return ROWS;
  }

  /** 长按弹出的字母：这一键的两个字母；L、M 只有一个字母，不弹。选中后先结束组字再上屏这个字母，同九键。 */
  static holdOptions(key: FourteenKey): string[] {
    return key.letters.length > 1 ? key.letters.split("") : [];
  }
}
