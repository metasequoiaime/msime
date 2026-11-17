import { KeyPressFlush } from "./KeyIdPolicy";

export interface TypingStatisticsKeysAction {
  operation: string;
  day: string;
  keys: Record<string, number>;
  sources: Record<string, Record<string, number>>;
}

export interface TypingStatisticsKeysRequest {
  directory: string;
  action: TypingStatisticsKeysAction;
}

/** 请求只携带已经固定的聚合快照，后台不读取当前模式。 */
export function typingKeyPressRequest(
  directory: string,
  flush: KeyPressFlush,
): TypingStatisticsKeysRequest {
  return {
    directory: directory,
    action: { operation: "record_keys", day: flush.day, keys: flush.keys, sources: flush.sources },
  };
}
