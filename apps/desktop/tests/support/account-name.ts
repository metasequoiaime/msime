import type { AccountNamePresentation } from "@msime/ui";

/** 只提供桥接结果替身；昵称规则由真实 Rust 测试覆盖。 */
export function accountNameResult(
  normalized: string,
  overrides: Partial<AccountNamePresentation> = {},
): AccountNamePresentation {
  return {
    normalized,
    preferred: normalized,
    valid: true,
    maximum_chars: 64,
    maximum_utf16_units: 128,
    ...overrides,
  };
}

export async function presentAccountName(name: string): Promise<AccountNamePresentation> {
  return accountNameResult(name);
}
