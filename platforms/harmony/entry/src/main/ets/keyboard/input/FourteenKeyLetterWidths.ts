import { FourteenKey } from "./FourteenKeyLayout";

/** 14/18 键三排字母格的共享宽度规则（#11051）。键距之外的空白格也带有所属键的动作。 */
export type FourteenKeyLetterWidthMode = "equal" | "varied";

export type FourteenKeyLetterWidthKind = "key" | "edge" | "pad";

export interface FourteenKeyLetterWidthCell {
  readonly kind: FourteenKeyLetterWidthKind;
  readonly key: FourteenKey | null;
  readonly weight: number;
  /** 占位格命中时转发给同排最外侧字母键。 */
  readonly owner: FourteenKey | null;
}

const SINGLE_LETTER_WEIGHT: number = 0.75;
const EPSILON: number = 0.0001;

/** 共享偏好 `touch_fourteen_key_letter_widths` 的严格取值；缺字段和未知值按现有等宽。 */
export function fourteenKeyLetterWidthMode(value: unknown): FourteenKeyLetterWidthMode {
  return value === "varied" ? "varied" : "equal";
}

function weight(key: FourteenKey, mode: FourteenKeyLetterWidthMode): number {
  if (mode === "equal" || key.letters.length > 1) return 1;
  return SINGLE_LETTER_WEIGHT;
}

function lettersWeight(row: readonly FourteenKey[], mode: FourteenKeyLetterWidthMode): number {
  return row.reduce((total: number, key: FourteenKey): number => total + weight(key, mode), 0);
}

/** 返回三排从左到右的格子；最后一排首尾为分词/删除边缘格。 */
export function fourteenKeyLetterWidthPlan(
  rows: readonly (readonly FourteenKey[])[],
  mode: FourteenKeyLetterWidthMode,
  edgeWeight: number,
): FourteenKeyLetterWidthCell[][] {
  if (rows.length === 0) return [];
  const first = lettersWeight(rows[0], mode);
  return rows.map(
    (keys: readonly FourteenKey[], rowIndex: number): FourteenKeyLetterWidthCell[] => {
      const last = rowIndex === rows.length - 1;
      const rest = mode === "varied" ? Math.max(0, (first - lettersWeight(keys, mode)) / 2) : 0;
      const cells: FourteenKeyLetterWidthCell[] = [];
      if (last) {
        cells.push({
          kind: "edge",
          key: null,
          weight: mode === "varied" ? rest : edgeWeight,
          owner: null,
        });
      } else if (rest > EPSILON) {
        cells.push({ kind: "pad", key: null, weight: rest, owner: keys[0] });
      }
      keys.forEach((key: FourteenKey): void => {
        cells.push({ kind: "key", key, weight: weight(key, mode), owner: null });
      });
      if (last) {
        cells.push({
          kind: "edge",
          key: null,
          weight: mode === "varied" ? rest : edgeWeight,
          owner: null,
        });
      } else if (rest > EPSILON) {
        cells.push({ kind: "pad", key: null, weight: rest, owner: keys[keys.length - 1] });
      }
      return cells;
    },
  );
}

export function fourteenKeyLetterSpacing(face: string): number {
  if (face.length <= 1) return 0;
  return face.includes(";") ? 0.08 : 0.15;
}
