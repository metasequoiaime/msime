import type { PreviewKey } from "./keyboard-layouts";

export type FourteenKeyLetterWidth = "equal" | "varied";

export const fourteenKeyLetterWidthOptions: readonly {
  value: FourteenKeyLetterWidth;
  label: string;
}[] = [
  { value: "equal", label: "等宽" },
  { value: "varied", label: "宽窄" },
];

export const fourteenKeyLetterWidthNote =
  "「宽窄」让单字母键窄一些、双字母键宽一些，第二排两端留空；点在空白里算最靠边的键。";

export function fourteenKeyLetterWidth(preferences: {
  touch_fourteen_key_letter_widths?: string;
}): FourteenKeyLetterWidth {
  return preferences.touch_fourteen_key_letter_widths === "varied" ? "varied" : "equal";
}

/** 预览使用与原生宿主相同的三排份额；空白格保留为无键的 PreviewKey。 */
export function withFourteenKeyLetterWidths(
  rows: readonly (readonly PreviewKey[])[],
  mode: FourteenKeyLetterWidth,
): PreviewKey[][] {
  if (mode === "equal" || rows.length === 0) return rows.map((row) => [...row]);
  const lettersWeight = (row: readonly PreviewKey[]) =>
    row.reduce((sum, key) => sum + (key.label.length > 1 ? key.weight : key.weight * 0.75), 0);
  const first = lettersWeight(rows[0]);
  return rows.map((row, rowIndex) => {
    const last = rowIndex === rows.length - 1;
    const letterCells = last ? row.slice(1, -1) : row;
    const rest = Math.max(0, (first - lettersWeight(letterCells)) / 2);
    const cells: PreviewKey[] = [];
    if (last) cells.push({ label: row[0]?.label ?? "", weight: rest, code: row[0]?.code });
    else if (rest > 0.0001) cells.push({ label: "", weight: rest });
    letterCells.forEach((key) =>
      cells.push({ ...key, weight: key.label.length > 1 ? key.weight : key.weight * 0.75 }),
    );
    if (last)
      cells.push({
        label: row[row.length - 1]?.label ?? "",
        weight: rest,
        code: row[row.length - 1]?.code,
      });
    else if (rest > 0.0001) cells.push({ label: "", weight: rest });
    return cells;
  });
}
