/** 九键双拼的键面和自定义码表文案，与 Android NineKeyShuangpinKeypad 保持同一契约。 */
import { TextPolicy } from "../TextPolicy";

export interface NineKeyShuangpinFace {
  readonly key: string;
  readonly initials: readonly string[];
  readonly finals: readonly string[];
  readonly zeroInitial: boolean;
}

export const NINE_KEY_SHUANGPIN_BUILTIN_PROFILES: readonly string[] = [
  "migala3",
  "migala2",
  "gushi2_low",
];
export const NINE_KEY_SHUANGPIN_DEFAULT_PROFILE = "migala3";
export const NINE_KEY_SHUANGPIN_CUSTOM_PROFILE = "custom";

export function normalizedNineKeyShuangpinProfile(value: string | null | undefined): string {
  return value === NINE_KEY_SHUANGPIN_CUSTOM_PROFILE ||
    NINE_KEY_SHUANGPIN_BUILTIN_PROFILES.includes(value ?? "")
    ? (value as string)
    : NINE_KEY_SHUANGPIN_DEFAULT_PROFILE;
}

export function nineKeyShuangpinProfileTitle(value: string | null | undefined): string {
  switch (normalizedNineKeyShuangpinProfile(value)) {
    case "migala2":
      return "米旮旯 2";
    case "gushi2_low":
      return "辜氏 2 低重码";
    case NINE_KEY_SHUANGPIN_CUSTOM_PROFILE:
      return "自定义";
    default:
      return "米旮旯 3";
  }
}

export function nineKeyShuangpinFace(
  faces: readonly NineKeyShuangpinFace[] | null | undefined,
  key: string,
): NineKeyShuangpinFace | null {
  return faces?.find((face) => face.key === key) ?? null;
}

function initialsLine(face: NineKeyShuangpinFace): string {
  const parts = face.initials.slice();
  if (face.zeroInitial) parts.unshift("零");
  return parts.join(" ");
}

export function nineKeyShuangpinLabel(face: NineKeyShuangpinFace): string {
  const initials = initialsLine(face);
  const finals = face.finals.join(" ");
  if (!initials) return finals;
  if (!finals) return initials;
  return `${initials}\n${finals}`;
}

export function nineKeyShuangpinCompactHint(face: NineKeyShuangpinFace): string {
  const initials = initialsLine(face);
  return initials || (face.finals.length > 0 ? "韵母" : "");
}

export function nineKeyShuangpinDescription(face: NineKeyShuangpinFace): string {
  let result = face.key;
  if (face.initials.length > 0 || face.zeroInitial) {
    result += "，声母";
    if (face.initials.length > 0) result += ` ${face.initials.join(" ")}`;
    if (face.zeroInitial) result += face.initials.length > 0 ? " 和零声母" : " 零声母";
  }
  if (face.finals.length > 0) result += `，韵母 ${face.finals.join(" ")}`;
  return result;
}

export function nineKeyShuangpinPartTitle(part: string | null | undefined): string {
  switch (part ?? "") {
    case "initials":
      return "声母";
    case "finals":
      return "韵母";
    case "zero_initials":
      return "零声母";
    default:
      return "";
  }
}

export function nineKeyShuangpinUnitLabel(unit: string | null | undefined): string {
  if (!unit) return "";
  return unit.startsWith("v") ? `ü${unit.slice(1)}` : unit;
}

export function validNineKeyShuangpinKey(
  part: string | null | undefined,
  key: string | null | undefined,
): boolean {
  if (!key) return false;
  const length = part === "zero_initials" ? 2 : 1;
  return (
    key.length === length &&
    TextPolicy.characters(key).every((digit) => digit >= "0" && digit <= "9")
  );
}

export function nineKeyShuangpinProblemMessage(
  kind: string | null | undefined,
  part: string | null | undefined,
  unit: string | null | undefined,
  key: string | null | undefined,
): string {
  const place = `${nineKeyShuangpinPartTitle(part)}${unit ? `「${nineKeyShuangpinUnitLabel(unit)}」` : ""}`;
  switch (kind ?? "") {
    case "unknown_unit":
      return `${place}不是这张表认识的单位`;
    case "duplicate_unit":
      return `${place}写了两次`;
    case "missing_unit":
      return `${place}还没有放到数字键上`;
    case "invalid_key":
      return `${place}的键「${key ?? ""}」不对：${part === "zero_initials" ? "零声母要两位数字" : "声母和韵母各是一位数字"}`;
    default:
      return "这张表不能用";
  }
}
