import { key, modifier, type KeyboardKey } from "./keyboard-input";
import type { NineKeyShuangpinKeyFace } from "../settings/nine-key-shuangpin-profile";

export const keyboardRows: KeyboardKey[][] = [
  [
    key("Num Lock", 0x90),
    key("Num 0", 0x60),
    key("Num 1", 0x61),
    key("Num 2", 0x62),
    key("Num 3", 0x63),
    key("Num 4", 0x64),
    key("Num 5", 0x65),
    key("Num 6", 0x66),
    key("Num 7", 0x67),
    key("Num 8", 0x68),
    key("Num 9", 0x69),
    key("Num *", 0x6a),
    key("Num +", 0x6b),
    key("Num -", 0x6d),
    key("Num /", 0x6f),
    key("Num .", 0x6e),
  ],
  [
    key("Esc", 0x1b),
    key("F1", 0x70),
    key("F2", 0x71),
    key("F3", 0x72),
    key("F4", 0x73),
    key("F5", 0x74),
    key("F6", 0x75),
    key("F7", 0x76),
    key("F8", 0x77),
    key("F9", 0x78),
    key("F10", 0x79),
    key("F11", 0x7a),
    key("F12", 0x7b),
    key("PrtSc", 0x2c),
    key("Scroll", 0x91),
    key("Pause", 0x13),
    key("Ins", 0x2d),
    key("Home", 0x24),
    key("End", 0x23),
    key("PgUp", 0x21),
    key("PgDn", 0x22),
  ],
  [
    key("`", 0xc0, "~"),
    ...[..."1234567890"].map((label, index) =>
      key(label, label.charCodeAt(0), ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")"][index]),
    ),
    key("-", 0xbd, "_"),
    key("=", 0xbb, "+"),
    key("Backspace", 0x08),
  ],
  [
    key("Tab", 0x09),
    ...[..."QWERTYUIOP"].map((label) => key(label.toLowerCase(), label.charCodeAt(0))),
    key("[", 0xdb, "{"),
    key("]", 0xdd, "}"),
    key("\\", 0xdc, "|"),
  ],
  [
    modifier("Caps Lock", 0x14),
    ...[..."ASDFGHJKL"].map((label) => key(label.toLowerCase(), label.charCodeAt(0))),
    key(";", 0xba, ":"),
    key("'", 0xde, '"'),
    key("Enter", 0x0d),
  ],
  [
    modifier("Shift", 0x10),
    ...[..."ZXCVBNM"].map((label) => key(label.toLowerCase(), label.charCodeAt(0))),
    key(",", 0xbc, "<"),
    key(".", 0xbe, ">"),
    key("/", 0xbf, "?"),
    modifier("Shift", 0x10),
  ],
  [
    modifier("Ctrl", 0x11),
    modifier("Win", 0x5b),
    modifier("Alt", 0x12),
    key("Space", 0x20, " "),
    modifier("Alt", 0x12),
    modifier("Win", 0x5b),
    key("Menu", 0x5d),
    key("Del", 0x2e),
    key("←", 0x25),
    key("↑", 0x26),
    key("↓", 0x28),
    key("→", 0x27),
    modifier("Ctrl", 0x11),
  ],
];

export const eighteenKeyRows: KeyboardKey[][] = [
  [..."123456789"].map((label) => key(label, label.charCodeAt(0))),
  ...["Q WE RT Y U IO P", "A SD FG H JK L", "Z XC V BN M"].map((row) =>
    row.split(" ").map((label) => ({
      label,
      virtualKey: label.charCodeAt(0),
      groupedLetter: label[0].toLowerCase(),
    })),
  ),
  [key("分词", 0xde), key("Backspace", 0x08), key("Space", 0x20), key("Enter", 0x0d)],
];

export const nineKeyRows: KeyboardKey[][] = [
  [key("1", 0x31), key("2", 0x32), key("3", 0x33)],
  [key("4", 0x34), key("5", 0x35), key("6", 0x36)],
  [key("7", 0x37), key("8", 0x38), key("9", 0x39)],
  [key("Backspace", 0x08), key("0", 0x30), key("Enter", 0x0d)],
  [key("Space", 0x20, " ")],
];

/** 把引擎返回的九键双拼键面叠到数字键上；实际发送仍是数字虚拟键。 */
export function nineKeyShuangpinRows(keys: readonly NineKeyShuangpinKeyFace[]): KeyboardKey[][] {
  const byKey = new Map(keys.map((item) => [item.key, item]));
  const face = (keyName: string): KeyboardKey => {
    const item = byKey.get(keyName);
    const hint = item
      ? [item.initials.join(" "), item.finals.join(" "), item.zero_initial ? "零声母" : ""]
          .filter(Boolean)
          .join(" · ")
      : undefined;
    return { label: keyName, hint, virtualKey: keyName.charCodeAt(0) };
  };
  return [
    [face("1"), face("2"), face("3")],
    [face("4"), face("5"), face("6")],
    [face("7"), face("8"), face("9")],
    [key("Backspace", 0x08), face("0"), key("Enter", 0x0d)],
    [key("Space", 0x20, " ")],
  ];
}
