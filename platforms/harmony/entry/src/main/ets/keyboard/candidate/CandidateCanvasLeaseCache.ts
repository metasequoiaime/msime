/** 一份按 key 共享的解码结果；`readers` 是此刻还在用它的视图数。 */
export interface CandidateCanvasLease {
  readers: number;
}

/**
 * 候选皮肤位图的引用计数缓存，最后一个读者离开后再停放一份。
 *
 * 手机上候选条是解码结果唯一的常驻读者，候选条一空（上屏、取消，联想默认关着）它就放手，下一个词的第一个字母又要重新解码整套皮肤，APNG 还要在 UI 线程上逐像素合成。所以读者归零时不立即释放，而是停放最近这一份：同一个 key 再被取用时直接接回去。停放最多一份，换皮肤、切深浅色（`readCandidateCanvas` 每次都换新 key）时被新 key 挤掉，不会让新旧两套位图同时驻留。键盘收起、编辑框停止输入和扩展销毁时 `trim()` 释放停放的那份，并且在下一次取用之前不再停放，收起之后才离开的读者直接释放，隐藏的键盘不占位图内存。
 *
 * 一份解码结果任一时刻只在 `live` 或停放位之一，释放只经 `free` 走一次。
 */
export class CandidateCanvasLeaseCache<T extends CandidateCanvasLease> {
  private live: Map<string, T> = new Map<string, T>();
  private parkedKey: string = '';
  private parked: T | null = null;
  private parking: boolean = true;
  private readonly free: (lease: T) => void;

  constructor(free: (lease: T) => void) {
    this.free = free;
  }

  /** 取用 `key` 的解码结果；既不在用也没停放时才调 `create` 新解码。 */
  retain(key: string, create: () => T): T {
    this.parking = true;
    let lease: T | undefined = this.live.get(key);
    if (lease === undefined) {
      if (this.parked !== null && this.parkedKey === key) {
        lease = this.parked;
        this.parked = null;
        this.parkedKey = '';
      } else {
        this.dropParked();
        lease = create();
      }
      this.live.set(key, lease);
    }
    lease.readers++;
    return lease;
  }

  release(key: string, lease: T): void {
    lease.readers--;
    if (lease.readers > 0) return;
    if (this.live.get(key) === lease) this.live.delete(key);
    if (!this.parking) {
      this.free(lease);
      return;
    }
    this.dropParked();
    this.parked = lease;
    this.parkedKey = key;
  }

  /** 释放停放的那份，下一次 `retain` 之前离开的读者不再停放。还有读者的解码结果不受影响。 */
  trim(): void {
    this.parking = false;
    this.dropParked();
  }

  private dropParked(): void {
    const parked: T | null = this.parked;
    this.parked = null;
    this.parkedKey = '';
    if (parked !== null) this.free(parked);
  }
}
