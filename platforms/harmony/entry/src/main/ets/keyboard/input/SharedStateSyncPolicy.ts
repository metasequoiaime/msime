/** 键盘获得焦点时共享状态操作的顺序。 */
export type SharedStateSyncAction = "import" | "drain" | "publish";

export class SharedStateSyncPolicy {
  static keyboardActions(fullMode: boolean): SharedStateSyncAction[] {
    return fullMode ? ["import", "drain", "publish"] : ["import"];
  }
}
