import type { AccountUser } from "./account-model";

export function preferredAccountName(user: AccountUser): string {
  const name = user.displayName.trim();
  return name || `水杉小鹿·${user.id.slice(0, 6).toUpperCase()}`;
}

export function accountProviderName(provider: string): string {
  if (provider === "apple") return "Apple";
  if (provider === "google") return "Google";
  if (provider === "email") return "邮箱";
  if (provider === "phone" || provider === "sms") return "手机号";
  return provider;
}
