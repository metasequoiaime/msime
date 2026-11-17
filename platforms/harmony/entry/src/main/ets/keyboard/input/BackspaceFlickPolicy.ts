/**
 * 删除键的四向 flick（#11096）：上滑清空光标前文本，下滑恢复最近一次清空，左滑删一个字符，右滑恢复最近一次单字符删除。
 * 判定只依赖窗口坐标，宿主在触摸取消时不执行动作。
 */
export enum BackspaceFlickAction {
  NONE = "none",
  CLEAR = "clear",
  RESTORE = "restore",
  DELETE_ONE = "delete-one",
  RESTORE_ONE = "restore-one",
}

export class BackspaceFlickPolicy {
  static readonly THRESHOLD_VP: number = 24;

  static action(
    dx: number,
    dy: number,
    threshold: number = BackspaceFlickPolicy.THRESHOLD_VP,
  ): BackspaceFlickAction {
    if (
      !Number.isFinite(dx) ||
      !Number.isFinite(dy) ||
      !Number.isFinite(threshold) ||
      threshold < 0 ||
      Math.max(Math.abs(dx), Math.abs(dy)) <= threshold
    ) {
      return BackspaceFlickAction.NONE;
    }
    if (Math.abs(dy) >= Math.abs(dx)) {
      return dy < 0 ? BackspaceFlickAction.CLEAR : BackspaceFlickAction.RESTORE;
    }
    return dx < 0 ? BackspaceFlickAction.DELETE_ONE : BackspaceFlickAction.RESTORE_ONE;
  }
}

/** 一次删除键触摸只判定一次，判出后滑回也不改动作。 */
export class BackspaceFlickGesture {
  private finger: number = -1;
  private downX: number = 0;
  private downY: number = 0;
  private decided: BackspaceFlickAction = BackspaceFlickAction.NONE;

  down(finger: number, x: number, y: number): void {
    this.cancel();
    this.finger = finger;
    this.downX = x;
    this.downY = y;
  }

  move(finger: number, x: number, y: number): BackspaceFlickAction {
    if (finger !== this.finger || this.decided !== BackspaceFlickAction.NONE) {
      return BackspaceFlickAction.NONE;
    }
    const next: BackspaceFlickAction = BackspaceFlickPolicy.action(x - this.downX, y - this.downY);
    if (next !== BackspaceFlickAction.NONE) this.decided = next;
    return next;
  }

  tracking(finger: number): boolean {
    return finger === this.finger;
  }

  up(finger: number): BackspaceFlickAction {
    if (finger !== this.finger) return BackspaceFlickAction.NONE;
    const result: BackspaceFlickAction = this.decided;
    this.cancel();
    return result;
  }

  cancel(): void {
    this.finger = -1;
    this.downX = 0;
    this.downY = 0;
    this.decided = BackspaceFlickAction.NONE;
  }
}

export interface BackspaceFlickTextSnapshot {
  readonly text: string;
  readonly cursor: number;
  readonly before: string;
  readonly after: string;
  readonly generation: number;
}

/** 供宿主行为测试复用的编辑器快照规则；游标按用户可见字符计数。 */
export class BackspaceFlickTextPolicy {
  static context(value: string, preceding: boolean, limit: number = 64): string {
    const characters: string[] = TextPolicy.characters(value);
    const length: number = Math.min(Math.max(limit, 0), characters.length);
    return preceding
      ? characters.slice(characters.length - length).join("")
      : characters.slice(0, length).join("");
  }

  static deleteBeforeCursor(
    value: string,
    cursor: number,
    count: number = 1,
  ): {
    readonly value: string;
    readonly deleted: string;
    readonly cursor: number;
  } {
    const characters: string[] = TextPolicy.characters(value);
    const end: number = Math.max(0, Math.min(cursor, characters.length));
    const start: number = Math.max(0, end - Math.max(0, count));
    return {
      value: characters.slice(0, start).concat(characters.slice(end)).join(""),
      deleted: characters.slice(start, end).join(""),
      cursor: start,
    };
  }

  static canRestore(
    snapshot: BackspaceFlickTextSnapshot,
    cursor: number,
    before: string,
    after: string,
    generation: number,
  ): boolean {
    return (
      snapshot.generation === generation &&
      snapshot.cursor === cursor &&
      snapshot.before === before &&
      snapshot.after === after
    );
  }
}
import { TextPolicy } from "../TextPolicy";
