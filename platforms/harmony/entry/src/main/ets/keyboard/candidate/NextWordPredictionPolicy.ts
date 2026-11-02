/**
 * 上屏后联想（#9433）在鸿蒙键盘上的几条规则；不依赖 ArkUI，便于在 node 下测试。
 *
 * 联想是引擎在选词上屏后给出的建议，View 里 `prediction` 为真、`editing_text` 为空。手机和 2in1 都和普通候选一样选：点选、数字键选本页对应的一条、空格选高亮的一条，这几样由共享运行时在交给引擎之前接管；回车、标点和退格照常输入，关闭键 × 和实体键盘的 Esc 关闭联想（送 Cancel，引擎清掉建议并报已处理，不往编辑器写字）。候选栏右端的按键样式来自共享偏好 `next_word_prediction_buttons`，写法与 client-core 的 `NextWordPredictionButtons`、Android 的 `PredictionButtonsPolicy` 相同。
 */
export enum PredictionButtons {
  /** 现有样式：只有展开键。缺省值，偏好文档里不写。 */
  EXPAND_ONLY = "expand_only",
  /** 关闭键 × 加展开键；展开面板打开时 × 仍在展开键左边。 */
  CLOSE_AND_EXPAND = "close_and_expand",
  /** 只有关闭键 ×，联想只能在候选栏里左右滑动选择。 */
  CLOSE_ONLY = "close_only",
}

export class NextWordPredictionPolicy {
  static readonly BUTTONS_PREFERENCE_KEY: string = "next_word_prediction_buttons";

  /** 宽松读取：缺键和不认识的写法（较新版本加的样式）都按现有样式。 */
  static buttons(value: string | undefined): PredictionButtons {
    if (value === PredictionButtons.CLOSE_AND_EXPAND) return PredictionButtons.CLOSE_AND_EXPAND;
    if (value === PredictionButtons.CLOSE_ONLY) return PredictionButtons.CLOSE_ONLY;
    return PredictionButtons.EXPAND_ONLY;
  }

  /** 候选栏是否放关闭键：只在显示联想时，并且样式带 ×。普通组字时永远不放。 */
  static showsClose(style: PredictionButtons, prediction: boolean): boolean {
    return prediction && style !== PredictionButtons.EXPAND_ONLY;
  }

  /** 展开键能不能出现：联想时「只有关闭」不放展开键，其余情况照旧。 */
  static allowsExpand(style: PredictionButtons, prediction: boolean): boolean {
    return !prediction || style !== PredictionButtons.CLOSE_ONLY;
  }

  /** 视图此刻是否在显示联想：View 标了 `prediction`，而且真有候选。 */
  static showing(prediction: boolean | undefined, candidateCount: number): boolean {
    return prediction === true && candidateCount > 0;
  }

  /** 回车要确认的候选有几个：联想的候选不算，回车在联想时照常换行或执行编辑器动作，键面不显示「确认」。空格不看这个：联想时空格照样选高亮的一条。 */
  static composingCandidates(candidateCount: number, prediction: boolean): number {
    return prediction ? 0 : candidateCount;
  }

  /** 2in1 候选窗该不该出现：有组字，或者在显示联想。联想没有组字，只看组字的话候选窗不出来，联想就成了看不见、却还在等点选的建议。 */
  static candidateWindowWanted(composing: boolean, showingPrediction: boolean): boolean {
    return composing || showingPrediction;
  }

  /** OpenHarmony 的 Esc 键码（`KeyCode.KEYCODE_ESCAPE`）。 */
  static readonly ESCAPE_KEYCODE: number = 2070;

  /** OpenHarmony 的空格键码（`KeyCode.KEYCODE_SPACE`）。 */
  static readonly SPACE_KEYCODE: number = 2050;

  /**
   * 实体键盘在联想时要交给运行时去选的键，返回交给 `msime_client_character` 的字符，别的键为 -1：不带修饰键的空格（选高亮的一条），以及「数字键选词」（`number_row_selection`）开着时不带修饰键的数字（选本页对应的一条）。没有组字时路由会把这些键交给应用，联想就选不中。越过本页的数字、九键和拿数字拼写的方案由运行时自己决定交回，交回的键照常给应用。回车、退格不在这里，照常交给应用。
   */
  static selectionKey(
    keyCode: number,
    unicodeChar: number,
    ctrlKey: boolean,
    altKey: boolean,
    logoKey: boolean,
    shiftKey: boolean,
    numberRowSelection: boolean,
  ): number {
    if (ctrlKey || altKey || logoKey || shiftKey) return -1;
    if (keyCode === NextWordPredictionPolicy.SPACE_KEYCODE) return 0x20;
    if (numberRowSelection && unicodeChar >= 0x30 && unicodeChar <= 0x39) return unicodeChar;
    return -1;
  }

  /** 实体键盘上关闭联想的键：不带修饰键的 Esc。 */
  static dismissesOnKey(
    keyCode: number,
    ctrlKey: boolean,
    altKey: boolean,
    logoKey: boolean,
    shiftKey: boolean,
  ): boolean {
    return (
      keyCode === NextWordPredictionPolicy.ESCAPE_KEYCODE &&
      !ctrlKey &&
      !altKey &&
      !logoKey &&
      !shiftKey
    );
  }

  /** 编辑器回报的改动不是键盘自己写入的回声（用户移了光标、应用改了文字、实体键交给应用后插进去的回车或退格），而且正显示联想时，重新聚焦会话，切断旧的词序上下文并清掉建议，与 Android 的 `onUpdateSelection` 相同。 */
  static refocusesOnEditorChange(ownEcho: boolean, showingPrediction: boolean): boolean {
    return !ownEcho && showingPrediction;
  }
}
