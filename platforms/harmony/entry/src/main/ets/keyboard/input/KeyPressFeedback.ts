/**
 * 按键区按下即振动（#10757）：手指落在按键区时就振动，抬手上屏时会话不再振第二次。规则与 Android 的 `KeyPressFeedbackPolicy` 相同（iOS 由 #10768 在按键自己的按下和松手里实现同一条规则），这里不依赖 ArkUI，node 下直接测。
 *
 * 按键的动作挂在点击上，点击要等手指抬起才发生；振动以前也在上屏路径里（`KeyboardSession.tapFeedback`），所以打字有「不跟手」的延迟。现在按键区的容器在每根手指按下时调用 `down` 给出振动，并为这次按下留一个令牌；上屏路径的振动先问 `consume`，只消耗本次松手绑定的令牌、不再振动。手指抬起或被取消时 `up` 交出它的令牌，会话在这次点击处理完之后（`RELEASE_GRACE_MS`）用 `settle` 撤回还没被消耗的令牌，所以没有上屏的按下（Shift、切层、滑出去取消）不会吞掉之后的振动。
 *
 * 令牌是每次按下新取的序号，不是手指 id：ArkUI 的手指 id 会复用，快速连点同一个位置时，上一次抬手排着的撤回不能撤掉这一次按下的令牌。长按这类确认性的振动（`KeyboardSession.holdFeedback`）不问这里，照常振动。
 */
export class KeyPressFeedback {
  /** 抬手后再等这么久才撤回令牌：ArkUI 的点击在抬手之后分发，留这点余量让这次点击先消耗到它。撤回得晚只会让这段时间里另一次没有按下的上屏（读屏点击）少振一下，撤回得早则会让这次上屏再振一次。 */
  static readonly RELEASE_GRACE_MS: number = 100;

  /** 还没被消耗、也没被撤回的令牌，按按下的先后排列。 */
  private readonly armed: number[] = [];
  /** 每根还按着的手指这次按下的令牌。 */
  private readonly fingers: Map<number, number> = new Map<number, number>();
  private nextToken: number = 1;
  /** 本次松手授予上屏路径的令牌；不能借用另一根仍按着的手指。 */
  private releasedToken: number | undefined = undefined;

  /** 一根手指在按键区按下：调用方这时振动，这里为这次按下留一个令牌。同一根手指的上一次按下没收到抬起时，它的令牌当场撤回。 */
  down(finger: number): void {
    const previous: number | undefined = this.fingers.get(finger);
    if (previous !== undefined) {
      this.settle(previous);
    }
    const token: number = this.nextToken;
    this.nextToken += 1;
    this.fingers.set(finger, token);
    this.armed.push(token);
  }

  /** 手指抬起或被取消：交出这次按下的令牌，调用方在点击处理完之后交给 `settle`；这根手指没在按键区按下过时为 undefined。 */
  up(finger: number): number | undefined {
    const token: number | undefined = this.fingers.get(finger);
    this.fingers.delete(finger);
    if (token !== undefined) {
      this.releasedToken = token;
    }
    return token;
  }

  /** 撤回一个令牌；已经被消耗或撤回过的什么也不做。 */
  settle(token: number | undefined): void {
    if (token === undefined) {
      return;
    }
    if (this.releasedToken === token) {
      this.releasedToken = undefined;
    }
    const index: number = this.armed.indexOf(token);
    if (index >= 0) {
      this.armed.splice(index, 1);
    }
  }

  /** 上屏只消费本次松手的令牌；按下顺序和松手顺序可以不同，尚未松手的按压不参与。 */
  consume(): boolean {
    const token: number | undefined = this.releasedToken;
    if (token === undefined || this.armed.indexOf(token) < 0) {
      return false;
    }
    this.settle(token);
    return true;
  }

  /** 视图换掉或键盘收起时丢掉所有令牌。 */
  clear(): void {
    this.armed.length = 0;
    this.fingers.clear();
    this.releasedToken = undefined;
  }

  /** 还没被消耗、也没被撤回的令牌个数。 */
  armedCount(): number {
    return this.armed.length;
  }
}
