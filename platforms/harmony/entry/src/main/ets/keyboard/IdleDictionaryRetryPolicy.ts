/** 第一次重试与原来一样两秒后进行，此后每次连续没有进展就翻倍。 */
export const IDLE_DICTIONARY_RETRY_MS: number = 2000;
/** 退避的上限：卡住的队列最多五分钟重建一次会话，而不是每两秒一次。 */
export const IDLE_DICTIONARY_RETRY_MAX_MS: number = 5 * 60 * 1000;

/** 一次个人词库排空的结果：`settled` 队列已空，`progress` 这次有进展，`stalled` 失败或原地不动。 */
export type PersonalDictionaryDrain = "settled" | "progress" | "stalled";

/**
 * 键盘空闲时重建 Engine 会话去处理词库队列（个人词库排空、云端词库快照），这里决定什么时候再试。
 *
 * 重建会话要在 ArkTS 主线程上重新准备宿主、打开词库，低端机上一次就是上百毫秒。队列一直推进不了时（整批调用被拒、属于账号的快照请求读不到会话），固定两秒重试会让键盘无休止地每两秒重建一次，手机发热耗电。所以只有真的有进展才按两秒继续，连续没有进展就按 2、4、8 秒……退避到五分钟。
 */
export class IdleDictionaryRetryPolicy {
  /** 连续 `stalls` 次没有进展之后，下一次重试等多久；0 或 1 次都是两秒，与原来的第一次重试一样快。 */
  static delay(stalls: number): number {
    let delay: number = IDLE_DICTIONARY_RETRY_MS;
    for (let index = 1; index < stalls && delay < IDLE_DICTIONARY_RETRY_MAX_MS; index++) {
      delay *= 2;
    }
    return Math.min(delay, IDLE_DICTIONARY_RETRY_MAX_MS);
  }

  /**
   * 判断一次成功返回的个人词库同步有没有进展。
   *
   * `pending` 是同步之后队列里还剩的条数，`sent` 是命名词库这次又送进队列的条数，`previousPending` 是上一次同步之后剩的条数（不知道时为负数）。送出了新的一批，或者剩下的比上次少，都算进展；剩下的条数原地不动又没有送出新的，就是卡住了。
   */
  static drain(pending: number, sent: number, previousPending: number): PersonalDictionaryDrain {
    if (pending === 0 && sent === 0) {
      return "settled";
    }
    if (sent > 0 || previousPending < 0 || pending < previousPending) {
      return "progress";
    }
    return "stalled";
  }
}
