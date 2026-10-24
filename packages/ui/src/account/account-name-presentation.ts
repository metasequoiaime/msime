import type { AccountNamePresentation } from "./account-model";

/** 只接受共享纯函数的完整结果；不在页面中重建昵称业务规则。 */
export function parseAccountNamePresentation(value: unknown): AccountNamePresentation {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw { code: "account_unavailable" };
  const result = value as Record<string, unknown>;
  const chars = result.maximum_chars;
  const units = result.maximum_utf16_units;
  if (
    typeof result.normalized !== "string" ||
    typeof result.preferred !== "string" ||
    typeof result.valid !== "boolean" ||
    /[\uD800-\uDFFF]/u.test(result.normalized) ||
    /[\uD800-\uDFFF]/u.test(result.preferred) ||
    typeof chars !== "number" ||
    !Number.isInteger(chars) ||
    chars <= 0 ||
    chars > 16_384 ||
    typeof units !== "number" ||
    !Number.isInteger(units) ||
    units < chars ||
    units > 16_384
  )
    throw { code: "account_unavailable" };
  return {
    normalized: result.normalized,
    preferred: result.preferred,
    valid: result.valid,
    maximum_chars: chars,
    maximum_utf16_units: units,
  };
}
