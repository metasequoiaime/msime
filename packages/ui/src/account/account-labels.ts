export function accountProviderName(provider: string): string {
  if (provider === "apple") return "Apple";
  if (provider === "google") return "Google";
  if (provider === "email") return "邮箱";
  if (provider === "phone" || provider === "sms") return "手机号";
  return provider;
}
