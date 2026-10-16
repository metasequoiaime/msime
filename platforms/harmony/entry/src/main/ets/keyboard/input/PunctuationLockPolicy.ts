/**
 * 固定标点（`punctuation_lock`）怎样压过中文标点开关。锁定为 chinese 或 english 时，共享层按锁定值出标点（host-api 的 `engine_chinese_punctuation`），所以工具栏的标点按钮和功能面板的开关也要显示锁定值；工具栏、Ctrl+. 的切换不起作用，也不改写会上云的 `chinese_punctuation`，与 Windows TIP 的 `ResolvePunctuationOpen`、fcitx5 的 `toggleChinesePunctuation` 相同。
 */
export class PunctuationLockPolicy {
  /** 实际输出的是不是中文标点：锁定时取锁定值，跟随时取开关。 */
  static effectiveChinese(chinesePunctuation: boolean, lock: string): boolean {
    if (lock === "chinese") return true;
    if (lock === "english") return false;
    return chinesePunctuation;
  }

  /** 切换中文标点开关有没有作用：只有跟随中英文状态时有。 */
  static toggles(lock: string): boolean {
    return lock !== "chinese" && lock !== "english";
  }
}
