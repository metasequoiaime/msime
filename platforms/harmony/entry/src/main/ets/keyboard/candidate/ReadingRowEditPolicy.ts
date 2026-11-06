import { LocalInputMode } from "../input/LocalInputMode";
import { CandidatePreeditStylePolicy, VisibleComposition } from "./CandidatePreeditStylePolicy";

/**
 * 手机候选栏读音行的「点读音编辑拼写」入口（#10591，笔记 2026-11-02-harmony-pronunciation-row-edit.md）。
 *
 * 拼写写在输入框里时读音行本来不画拼写（见 InlinePreeditPolicy.hidesStripReading）。可编辑时这一行用弱化的颜色画出拼写作为提示，用户看得见能点什么；点开之后照常画出拼写和分段光标，后续编辑仍走现有组字命令。2in1 候选窗口不受影响，照旧按「候选窗口预编辑」。
 */
export class ReadingRowEditPolicy {
  /** 读音行能不能点开：只在手机上、仍在组字、拼写确实写在输入框里、且方案允许移动组字光标时。 */
  static editable(
    desktop: boolean,
    compositionInField: boolean,
    editing: string,
    locksCaret: boolean,
    localMode: string = "",
    phrasePrefix: string = "",
  ): boolean {
    const mode = LocalInputMode.fromPreferenceKey(localMode);
    const spelling: string = editing.startsWith(phrasePrefix)
      ? editing.substring(phrasePrefix.length)
      : editing;
    return (
      !desktop &&
      compositionInField &&
      spelling.length > 0 &&
      !locksCaret &&
      (mode === null || spelling !== mode.trigger)
    );
  }

  /** 读音行画出来的组字。2in1 照「候选窗口预编辑」；手机在拼写写不进输入框、或读音行已点开时画拼写。 */
  static visible(
    desktop: boolean,
    desktopShowPreedit: boolean,
    compositionInField: boolean,
    expanded: boolean,
    phrasePrefix: string,
    editing: string,
    localMode: string,
  ): VisibleComposition {
    const showPreedit: boolean = desktop ? desktopShowPreedit : expanded || !compositionInField;
    return CandidatePreeditStylePolicy.visible(showPreedit, phrasePrefix, editing, localMode);
  }

  /** 可编辑但还没点开时，接在 `shown.text` 后面弱化画出的拼写；不需要提示、或拼写已经整段画出来时为空。 */
  static hint(
    editable: boolean,
    expanded: boolean,
    phrasePrefix: string,
    editing: string,
    shown: VisibleComposition,
  ): string {
    if (!editable || expanded || shown.caret) {
      return "";
    }
    const prefix: string = phrasePrefix ?? "";
    const text: string = editing ?? "";
    const spelling: string = text.startsWith(prefix) ? text.substring(prefix.length) : text;
    return shown.text === prefix + spelling ? "" : spelling;
  }

  /** 每次引擎回报之后点开状态还留不留：读音行不再可编辑（组字结束、离开行内组字、换成锁定光标的方案）就收起。 */
  static keepsExpanded(expanded: boolean, editable: boolean): boolean {
    return expanded && editable;
  }
}
