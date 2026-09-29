/** Provider-specific authentication headers shared by AI test and polish requests. */
export class AiAuthenticationPolicy {
  static isAnthropic(endpoint: string): boolean {
    const trimmed: string = endpoint.trim();
    if (!trimmed.toLowerCase().startsWith("https://")) return false;
    const authority: string = trimmed.substring(8).split(/[/?#]/, 1)[0];
    return /^api\.anthropic\.com(?::\d+)?$/i.test(authority);
  }

  static headers(endpoint: string, token: string): Record<string, string> {
    if (this.isAnthropic(endpoint)) {
      return { Accept: "application/json", "x-api-key": token, "anthropic-version": "2023-06-01" };
    }
    return { Accept: "application/json", Authorization: `Bearer ${token}` };
  }
}
