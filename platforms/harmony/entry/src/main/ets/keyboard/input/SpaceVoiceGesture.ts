/**
 * 空格键的按住说话判定，移植自 Android 的 `SpaceGesturePolicy`：按住 450 ms 开始语音，水平拖动超过阈值进入光标移动，先满足的一方胜出，之后另一方不再生效；两者都没发生时松手就是普通空格。
 *
 * 在此之上加了按住说话的收尾：语音开始后手指一直按着，松手结束录音并给出结果；手指上滑超过键区高度的 `SPACE_VOICE_CANCEL_FRACTION` 进入「松开取消」，在那里松手放弃这次录音，滑回来又恢复为松手结束；系统取消触摸时一律放弃。
 *
 * 纯状态机，不持有计时器：调用方按下时 `down`，移动时 `move`，计时回调里 `tick`，松手时 `up`，触摸被取消时 `cancel`。坐标与长度都用 vp。
 */

/** 长按开始语音的时长，与 Android `SpaceGesturePolicy.LONG_PRESS_MS` 相同。 */
export const SPACE_LONG_PRESS_MS: number = 450;
/** 水平拖动进入光标移动的阈值（vp），与 Android `SpaceGesturePolicy.DRAG_THRESHOLD_DP` 相同。 */
export const SPACE_DRAG_THRESHOLD_VP: number = 10;
/** 录音中手指上滑超过键区高度的这个比例即进入「松开取消」。 */
export const SPACE_VOICE_CANCEL_FRACTION: number = 0.25;

export enum SpaceGestureState {
  IDLE,
  PRESSED,
  VOICE,
  CURSOR,
}

/** 松手或触摸被取消时这次手势的结果。 */
export enum SpaceGestureOutcome {
  /** 输入一个空格。 */
  SPACE,
  /** 拖动过光标。 */
  CURSOR,
  /** 结束录音并给出结果。 */
  VOICE_FINISH,
  /** 放弃这次录音。 */
  VOICE_CANCEL,
  NONE,
}

export class SpaceVoiceGesture {
  private current: SpaceGestureState = SpaceGestureState.IDLE;
  private downAt: number = 0;
  private downX: number = 0;
  private downY: number = 0;
  /** 最近一次知道的手指纵坐标：计时回调进入语音时没有坐标，靠它判断手指是否已经在取消区。 */
  private lastY: number = 0;
  private cancelDistance: number = 0;
  private voiceArmed: boolean = false;
  private inCancelZone: boolean = false;

  /**
   * 这次按压能否长按进入语音，即 Android `voiceInsertionReady` 的条件加上触屏：组字中或本地模式下空格有自己的用途，长按不该把它吞掉；已经在录音时也不再开新的；2in1 的语音由快捷键和语音面板承担。
   */
  static armed(touch: boolean, voiceEnabled: boolean, recording: boolean, composing: boolean,
    localMode: string): boolean {
    return touch && voiceEnabled && !recording && !composing && localMode === "none";
  }

  state(): SpaceGestureState {
    return this.current;
  }

  /** 录音中手指是否停在「松开取消」区域。 */
  cancelling(): boolean {
    return this.current === SpaceGestureState.VOICE && this.inCancelZone;
  }

  /**
   * 按下。`keyboardHeight` 是键区高度，用来换算取消区；`voiceArmed` 为 false 时（组字中、本地模式、语音关闭或正在录音）长按不进入语音，停顿后拖动仍可移光标、松手仍是空格。
   */
  down(nowMs: number, x: number, y: number, keyboardHeight: number, voiceArmed: boolean): void {
    this.current = SpaceGestureState.PRESSED;
    this.downAt = nowMs;
    this.downX = x;
    this.downY = y;
    this.lastY = y;
    this.cancelDistance = Number.isFinite(keyboardHeight) && keyboardHeight > 0
      ? keyboardHeight * SPACE_VOICE_CANCEL_FRACTION : Number.POSITIVE_INFINITY;
    this.voiceArmed = voiceArmed;
    this.inCancelZone = false;
  }

  /** 时间推进；返回这一刻是否刚好进入语音（只在进入的那一次为 true）。 */
  tick(nowMs: number): boolean {
    if (!this.voiceArmed || this.current !== SpaceGestureState.PRESSED) return false;
    if (nowMs - this.downAt < SPACE_LONG_PRESS_MS) return false;
    this.current = SpaceGestureState.VOICE;
    this.inCancelZone = this.downY - this.lastY > this.cancelDistance;
    return true;
  }

  /** 手指移动；返回这次移动是否让空格进入光标移动（只在进入的那一次为 true）。录音中只更新是否处在取消区。 */
  move(nowMs: number, x: number, y: number): boolean {
    this.lastY = y;
    if (this.current === SpaceGestureState.VOICE) {
      this.inCancelZone = this.downY - y > this.cancelDistance;
      return false;
    }
    if (this.current !== SpaceGestureState.PRESSED) return false;
    if (this.tick(nowMs)) return false;
    if (Math.abs(x - this.downX) > SPACE_DRAG_THRESHOLD_VP) {
      this.current = SpaceGestureState.CURSOR;
      return true;
    }
    return false;
  }

  /**
   * 松手，返回结果并回到空闲。不在这里补 `tick`：计时回调还没跑就松手时，语音其实没有开始，按住说话的录音也就没有可结束的内容，此时仍当作普通空格。
   */
  up(): SpaceGestureOutcome {
    const outcome: SpaceGestureOutcome = this.current === SpaceGestureState.PRESSED
      ? SpaceGestureOutcome.SPACE
      : this.current === SpaceGestureState.CURSOR
        ? SpaceGestureOutcome.CURSOR
        : this.current === SpaceGestureState.VOICE
          ? (this.inCancelZone ? SpaceGestureOutcome.VOICE_CANCEL : SpaceGestureOutcome.VOICE_FINISH)
          : SpaceGestureOutcome.NONE;
    this.reset();
    return outcome;
  }

  /** 触摸被系统取消（例如窗口失焦）：录音中一律放弃，否则什么都不做。 */
  cancel(): SpaceGestureOutcome {
    const outcome: SpaceGestureOutcome = this.current === SpaceGestureState.VOICE
      ? SpaceGestureOutcome.VOICE_CANCEL : SpaceGestureOutcome.NONE;
    this.reset();
    return outcome;
  }

  private reset(): void {
    this.current = SpaceGestureState.IDLE;
    this.voiceArmed = false;
    this.inCancelZone = false;
  }
}
