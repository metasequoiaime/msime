/** 保留 Error 的原始消息（包括空字符串），其他抛出值使用调用方指定的提示。 */
export function exceptionMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}
