import { FourteenKey } from "./FourteenKeyLayout";
import { GridLetterHintTable } from "./GridLetterHintTable";
import { GridKeyCommit, GridKeyGestureMode, GridKeyGesturePolicy } from "./GridKeyGesturePolicy";
import { HintKeyRect } from "./LetterHintGesture";
import { SwipeSymbolsDirection } from "./SwipeHintPolicy";

/** 一根手指在窗口坐标里的位置，单位 vp。 */
export interface GridKeyPoint {
  id: number;
  x: number;
  y: number;
}

/** 视图替控制器做的事：读设置、画键面和气泡、把输入交给会话。这里不涉及 ArkUI 和 NAPI。 */
export interface GridKeyTouchHost {
  readonly composing: () => boolean;
  /** 「滑动输入符号」的方向；关闭时为 null。 */
  readonly swipeDirection: () => SwipeSymbolsDirection | null;
  /** 这个键此刻是否接横滑选精确字母。 */
  readonly exactSwipes: (letters: string) => boolean;
  readonly keyPopup: () => boolean;
  /** 键面上画的字母。 */
  readonly keyFace: (key: FourteenKey) => string;
  /** 角标此刻的键面（中文标点、直角引号等设置之后），没有角标时为 null。 */
  readonly hintFace: (key: FourteenKey, eighteen: boolean) => string | null;
  readonly showPreview: (finger: number, label: string, rect: HintKeyRect) => void;
  readonly hidePreview: (finger: number) => void;
  readonly showLetter: (letters: string, letter: string) => void;
  readonly endLetter: (letters: string) => void;
  readonly keyUp: (finger: number) => void;
  /** 普通点按：记这一键并送整组。 */
  readonly tap: (key: FourteenKey, eighteen: boolean) => void;
  /** 记这一键并输入角标。 */
  readonly typeHint: (key: FourteenKey, eighteen: boolean, hint: string) => void;
  /** 横滑选中的精确字母，空串表示不输入。 */
  readonly typeLetter: (key: FourteenKey, eighteen: boolean, letter: string) => void;
}

/**
 * 14/18 键字母键的触摸、点击和长按分派：每个键一个 `GridKeyGesturePolicy`，按键上的字母（`FourteenKey.letters`）区分，两个拇指按在两个键上互不影响。视图只把 ArkUI 事件换成手指坐标转进来。
 */
export class GridKeyTouchController {
  private readonly host: GridKeyTouchHost;
  private readonly gestures: Map<string, GridKeyGesturePolicy> = new Map<
    string,
    GridKeyGesturePolicy
  >();

  constructor(host: GridKeyTouchHost) {
    this.host = host;
  }

  down(key: FourteenKey, eighteen: boolean, finger: GridKeyPoint, rect: HintKeyRect): void {
    const current: GridKeyGesturePolicy | undefined = this.gestures.get(key.letters);
    if (current !== undefined && current.secondFinger(finger.id)) {
      // 同一个键上落下第二根手指：整次按压作废。
      current.cancel();
      this.host.endLetter(key.letters);
      return;
    }
    const hint: string | null = GridLetterHintTable.hint(key.letters, eighteen);
    const gesture: GridKeyGesturePolicy = new GridKeyGesturePolicy(
      this.host.exactSwipes(key.letters) ? key.letters : null,
      hint !== null,
    );
    gesture.start(finger.id, finger.x, finger.y, this.host.swipeDirection(), this.host.composing());
    this.gestures.set(key.letters, gesture);
    if (this.host.keyPopup()) this.host.showPreview(finger.id, this.host.keyFace(key), rect);
  }

  move(key: FourteenKey, eighteen: boolean, fingers: GridKeyPoint[], rect: HintKeyRect): void {
    const gesture: GridKeyGesturePolicy | undefined = this.gestures.get(key.letters);
    if (gesture === undefined) return;
    this.composingSince(gesture);
    for (const finger of fingers) {
      const before: GridKeyGestureMode = gesture.mode();
      const mode: GridKeyGestureMode = gesture.move(finger.id, finger.x, finger.y);
      if (mode === GridKeyGestureMode.LETTER) {
        this.host.showLetter(key.letters, gesture.selectedLetter());
      }
      if (mode !== before && mode === GridKeyGestureMode.SYMBOL) {
        const face: string =
          this.host.hintFace(key, eighteen) ?? GridLetterHintTable.hint(key.letters, eighteen) ?? "";
        this.host.showPreview(finger.id, face, rect);
      }
    }
  }

  up(key: FourteenKey, eighteen: boolean, finger: GridKeyPoint): void {
    this.host.hidePreview(finger.id);
    this.host.endLetter(key.letters);
    this.host.keyUp(finger.id);
    const gesture: GridKeyGesturePolicy | undefined = this.gestures.get(key.letters);
    if (gesture === undefined) return;
    this.composingSince(gesture);
    this.commit(key, eighteen, gesture, gesture.up(finger.id, finger.x, finger.y));
  }

  cancel(key: FourteenKey, fingers: number[]): void {
    this.gestures.get(key.letters)?.cancel();
    this.gestures.delete(key.letters);
    this.host.endLetter(key.letters);
    for (const finger of fingers) this.host.hidePreview(finger);
  }

  click(key: FourteenKey, eighteen: boolean, x: number, y: number): void {
    const gesture: GridKeyGesturePolicy | undefined = this.gestures.get(key.letters);
    if (gesture === undefined) {
      this.host.tap(key, eighteen);
      return;
    }
    this.composingSince(gesture);
    const commit: GridKeyCommit = gesture.click(x, y);
    if (commit === GridKeyCommit.LETTER || commit === GridKeyCommit.SYMBOL) {
      this.host.endLetter(key.letters);
    }
    this.commit(key, eighteen, gesture, commit);
  }

  hold(key: FourteenKey, eighteen: boolean): void {
    const hint: string | null = GridLetterHintTable.hint(key.letters, eighteen);
    const gesture: GridKeyGesturePolicy | undefined = this.gestures.get(key.letters);
    if (hint === null || gesture === undefined || !gesture.hold()) return;
    this.host.typeHint(key, eighteen, hint);
  }

  /** 触摸下的键被替换或键盘换了方案：丢弃全部按压。 */
  clear(): void {
    this.gestures.clear();
  }

  /** 按压中途另一根手指开始了组字：还没触发手势时，之后的纵滑按点按处理（Android `ImeLetterRows` 同样在按下之后的每个事件里检查）。 */
  private composingSince(gesture: GridKeyGesturePolicy): void {
    if (this.host.composing()) gesture.composing();
  }

  private commit(
    key: FourteenKey,
    eighteen: boolean,
    gesture: GridKeyGesturePolicy,
    commit: GridKeyCommit,
  ): void {
    const hint: string | null = GridLetterHintTable.hint(key.letters, eighteen);
    if (commit === GridKeyCommit.SYMBOL && hint !== null) {
      this.host.typeHint(key, eighteen, hint);
    } else if (commit === GridKeyCommit.LETTER) {
      this.host.typeLetter(key, eighteen, gesture.selectedLetter());
    } else if (commit === GridKeyCommit.GROUP) {
      this.host.tap(key, eighteen);
    }
  }
}
