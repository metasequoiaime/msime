/** 设置页「测试」被服务拒绝时的一句话，与共享 Rust 的 `credential::probe::rejected_status_message` 相同：带上 HTTP 状态码，不带服务端正文。402 是 DeepSeek、OpenRouter 等的余额不足，不该让用户去改接口地址和模型。5xx 多半是服务端故障，先让用户稍后重试；但 one-api、new-api 这类中转对没有渠道的模型名也回 503，所以重试无效时要提醒核对模型。`other` 是其余状态码时的建议。 */
export class CredentialStatusPolicy {
  /** 对话接口测试回 404 时的一句话，与共享 Rust 的 `credential::probe::CHAT_NOT_FOUND_MESSAGE` 相同。基础地址缺了 /chat/completions、模型名不存在或未开通，服务都回 404（#10876），要说清两处各该填什么。转写测试的 404 不用它。 */
  static readonly CHAT_NOT_FOUND: string =
    "服务找不到所填的接口地址或模型（HTTP 404）。接口地址要填完整的对话接口，通常以 /chat/completions 结尾；模型要填服务商控制台列出的模型 ID，并确认账号已开通该模型。";

  /** 对话接口测试被拒时的一句话：404 用 `CHAT_NOT_FOUND`，其余同 `rejected`。 */
  static chatRejected(status: number): string {
    if (status === 404) return CredentialStatusPolicy.CHAT_NOT_FOUND;
    return CredentialStatusPolicy.rejected(status, "服务拒绝请求，请检查接口地址和模型配置");
  }

  static rejected(status: number, other: string): string {
    if (status === 401 || status === 403)
      return `认证失败（HTTP ${status}），请检查 API Key 和访问权限。`;
    if (status === 402) return "账户余额不足（HTTP 402），请到服务商控制台充值后重试。";
    if (status === 429) return "服务限流或额度不足（HTTP 429），请稍后重试。";
    if (status >= 500 && status <= 599)
      return `服务暂时不可用（HTTP ${status}），请稍后重试；一直如此时，请确认该服务支持所填的模型。`;
    return `${other}（HTTP ${status}）`;
  }
}
