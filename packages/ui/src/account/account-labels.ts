import type { AccountUser } from "./account-page";

const MAX_ACCOUNT_NAME_LENGTH = 64;

export function preferredAccountName(user: AccountUser): string {
  const name = user.displayName.trim();
  return name || `水杉小鹿·${user.id.slice(0, 6).toUpperCase()}`;
}

export function normalizeAccountName(value: string): string {
  return value.trim();
}

export function isValidAccountName(value: string): boolean {
  const normalized = normalizeAccountName(value);
  return (
    Boolean(normalized) &&
    [...normalized].length <= MAX_ACCOUNT_NAME_LENGTH &&
    !/[\u0000-\u001f\u007f]/.test(normalized)
  );
}

export function accountProviderName(provider: string): string {
  if (provider === "apple") return "Apple";
  if (provider === "google") return "Google";
  if (provider === "email") return "邮箱";
  if (provider === "phone" || provider === "sms") return "手机号";
  return provider;
}
