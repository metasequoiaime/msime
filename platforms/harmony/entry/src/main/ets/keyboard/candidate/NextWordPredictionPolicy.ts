/**
 * 上屏后联想（#9433）在鸿蒙键盘上的几条规则；不依赖 ArkUI，便于在 node 下测试。
 *
 * 联想是引擎在选词上屏后给出的建议，View 里 `prediction` 为真、`editing_text` 为空。它只是建议：只有点选候选才上屏，空格、回车、数字和标点照常输入，关闭键 × 和实体键盘的 Esc 关闭联想（送 Cancel，引擎清掉建议并报已处理，不往编辑器写字）。候选栏右端的按键样式来自共享偏好 `next_word_prediction_buttons`，写法与 client-core 的 `NextWordPredictionButtons`、Android 的 `PredictionButtonsPolicy` 相同。
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

  /** 属于组字的候选有几个：联想的候选不算，回车、空格的键面和读屏名称据此按空闲处理，不显示「确认」「选定」。 */
  static composingCandidates(candidateCount: number, prediction: boolean): number {
    return prediction ? 0 : candidateCount;
  }

  /** 2in1 候选窗该不该出现：有组字，或者在显示联想。联想没有组字，只看组字的话候选窗不出来，联想就成了看不见、却还在等点选的建议。 */
  static candidateWindowWanted(composing: boolean, showingPrediction: boolean): boolean {
    return composing || showingPrediction;
  }

  /** OpenHarmony 的 Esc 键码（`KeyCode.KEYCODE_ESCAPE`）。 */
  static readonly ESCAPE_KEYCODE: number = 2070;

  /** 实体键盘上关闭联想的键：不带修饰键的 Esc。空格、回车、数字和标点照常输入，不在这里。 */
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

  /** 编辑器回报的改动不是键盘自己写入的回声（用户移了光标、应用改了文字、实体键交给应用后插进去的空格或回车），而且正显示联想时，重新聚焦会话，切断旧的词序上下文并清掉建议，与 Android 的 `onUpdateSelection` 相同。 */
  static refocusesOnEditorChange(ownEcho: boolean, showingPrediction: boolean): boolean {
    return !ownEcho && showingPrediction;
  }
}
