/** 设置页「测试」被服务拒绝时的一句话，与共享 Rust 的 `credential::probe::rejected_status_message` 相同：带上 HTTP 状态码，不带服务端正文。402 是 DeepSeek、OpenRouter 等的余额不足，不该让用户去改接口地址和模型。5xx 多半是服务端故障，先让用户稍后重试；但 one-api、new-api 这类中转对没有渠道的模型名也回 503，所以重试无效时要提醒核对模型。`other` 是其余状态码时的建议。 */
export class CredentialStatusPolicy {
  static rejected(status: number, other: string): string {
    if (status === 401 || status === 403) return `认证失败（HTTP ${status}），请检查 API Key 和访问权限。`;
    if (status === 402) return "账户余额不足（HTTP 402），请到服务商控制台充值后重试。";
    if (status === 429) return "服务限流或额度不足（HTTP 429），请稍后重试。";
    if (status >= 500 && status <= 599) return `服务暂时不可用（HTTP ${status}），请稍后重试；一直如此时，请确认该服务支持所填的模型。`;
    return `${other}（HTTP ${status}）`;
  }
}
