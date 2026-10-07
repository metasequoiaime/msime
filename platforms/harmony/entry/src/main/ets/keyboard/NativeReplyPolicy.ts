/** 原生成功信封的 value 不能缺失；null 由各调用方按协议语义另行处理。 */
export class NativeReplyPolicy {
  static hasValue<T>(value: T | null | undefined): value is T {
    return value !== undefined && value !== null;
  }

  /** 只有成功且带有非空 value 的信封才能进入后续业务逻辑。 */
  static successfulValue<T>(reply: unknown): T | null {
    if (reply === null || typeof reply !== 'object' || Array.isArray(reply)) return null;
    const value: unknown = (reply as { ok?: unknown; value?: unknown }).value;
    return (reply as { ok?: unknown }).ok === true && NativeReplyPolicy.hasValue(value)
      ? value as T : null;
  }
}
