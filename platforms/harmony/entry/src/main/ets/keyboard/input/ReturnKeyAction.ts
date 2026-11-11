import { SchemeTraits } from "../SchemeTraits";

/**
 * Editor-action labels shared by return-key rendering and dispatch, ported from
 * platforms/android/java/app/msime/android/ReturnKeyAction.java.
 *
 * The numbers are the editor action constants the host receives; they are kept rather than renamed so
 * the mapping stays checkable against what the framework sends.
 */
const GO: number = 2;
const SEARCH: number = 3;
const SEND: number = 4;
const NEXT: number = 5;
const DONE: number = 6;
const PREVIOUS: number = 7;

export enum ReturnDispatch {
  EDITOR = "editor",
  FINISH_COMPOSITION = "finish-composition",
  COMMIT_HIGHLIGHTED = "commit-highlighted",
  COMMIT_READING = "commit-reading",
  /** Korean and Vietnamese: commit the open composition, then let Return do what it does in the editor. */
  FINISH_THEN_EDITOR = "finish-then-editor",
  /** Stroke and the letter-code schemes (`commitsSpelling`): commit the typed letters (MSIME_COMMIT_RAW), whatever the candidates. */
  COMMIT_RAW = "commit-raw",
}

export class ReturnKeyAction {
  /**
   * 组字时回车是否把键入的编码原样上屏（#11120）：只有编码本身就是用户想写的拉丁字母时才是，即全拼、双拼、五笔、粤拼的 26 键。笔画也上屏字母，但它有自己的分支，不经这里。
   *
   * 其余组字的原文不是用户键入的文字，回车照旧取候选或结束组字：九键和九键双拼的原文是数字串（`64426`），14 键和 18 键的是每组的代表字母，日语 26 键的是罗马字（`nihon`，用户要写的是假名），注音的回车归引擎在转换结果和打开的候选列表之间决定。`rulesScheme` 是 `SchemeCompositionPolicy.selectedRulesScheme` 的结果，英文和本地工具模式是 -1，不归方案的规则管。iOS 的 `CompositionBoundaryPolicy` 对回车排除的是同一批组字。
   */
  static commitsSpelling(
    rulesScheme: number,
    nineKeyFace: boolean,
    letterGridFace: boolean,
  ): boolean {
    return (
      !nineKeyFace &&
      !letterGridFace &&
      [
        SchemeTraits.QUANPIN,
        SchemeTraits.SHUANGPIN,
        SchemeTraits.WUBI,
        SchemeTraits.CANTONESE,
      ].includes(rulesScheme)
    );
  }

  static dispatch(
    japanese: boolean,
    composing: boolean,
    candidateCount: number,
    japaneseConverted: boolean = false,
    korean: boolean = false,
    vietnamese: boolean = false,
    stroke: boolean = false,
    prediction: boolean = false,
    tibetan: boolean = false,
    spelling: boolean = false,
  ): ReturnDispatch {
    // 上屏后联想只是建议：回车照常换行或执行编辑器动作。先结束联想（引擎对联想的 FINISH_COMPOSITION 清掉建议、报未处理，什么也不上屏），再交给编辑器；按候选数走「上屏高亮」的话，引擎清掉建议却不换行，这一下回车就被吞了。
    if (prediction) {
      return ReturnDispatch.FINISH_THEN_EDITOR;
    }
    // Stroke's Return commits the typed letters even with candidates on the strip, as a hardware Return (MSIME_COMMIT_RAW) does here and the iOS and Android keyboards do; Space is what picks the highlighted character.
    if (stroke) {
      return composing ? ReturnDispatch.COMMIT_RAW : ReturnDispatch.EDITOR;
    }
    // A Vietnamese word is finished text with no list behind it, so Return commits it and still breaks the line or submits, as for a Korean syllable.
    if (vietnamese) {
      return composing ? ReturnDispatch.FINISH_THEN_EDITOR : ReturnDispatch.EDITOR;
    }
    // 藏文的触屏回车只提交藏文，不执行编辑器动作；它的 Engine 组字不是普通编码串，不能套用中文回车的原样上屏。
    if (tibetan) {
      return composing ? ReturnDispatch.FINISH_COMPOSITION : ReturnDispatch.EDITOR;
    }
    // A Korean syllable is finished text rather than a spelling to confirm, so Return commits it and still breaks the line or submits, as every Korean keyboard does. The one exception is the syllable's open Hanja list, the only candidates Korean has: there Return chooses the highlighted Hanja, which only the session knows (msime_client.h).
    if (korean) {
      if (!composing) {
        return ReturnDispatch.EDITOR;
      }
      return candidateCount > 0
        ? ReturnDispatch.COMMIT_HIGHLIGHTED
        : ReturnDispatch.FINISH_THEN_EDITOR;
    }
    if (japanese && composing) {
      return japaneseConverted ? ReturnDispatch.COMMIT_HIGHLIGHTED : ReturnDispatch.COMMIT_READING;
    }
    // 字母编码方案（`commitsSpelling`）组字时的确认键对应 `MSIME_COMMIT_RAW`：空格和候选点选才是取首选，回车把用户键入的字母原样上屏。鸿蒙之前一律按候选数取首选，确认键没法把编码当英文输入。
    if (spelling && composing) {
      return ReturnDispatch.COMMIT_RAW;
    }
    // 其余组字（九键、14/18 键、日语 26 键、注音、英文和本地工具模式）的原文不是用户想写的字：有候选时上屏高亮行（注音只有候选列表打开时才有候选，选中后组字保留），没有候选时结束组字。
    if (candidateCount > 0) {
      return ReturnDispatch.COMMIT_HIGHLIGHTED;
    }
    return composing ? ReturnDispatch.FINISH_COMPOSITION : ReturnDispatch.EDITOR;
  }

  static performsEditorAction(action: number, disabled: boolean): boolean {
    if (disabled) {
      return false;
    }
    return (
      action === GO ||
      action === SEARCH ||
      action === SEND ||
      action === NEXT ||
      action === DONE ||
      action === PREVIOUS
    );
  }

  /** A handled composition consumes Return before any editor action or newline is considered. */
  static shouldPerformEditorAction(
    action: number,
    disabled: boolean,
    compositionHandled: boolean,
  ): boolean {
    return !compositionHandled && ReturnKeyAction.performsEditorAction(action, disabled);
  }

  static title(action: number, disabled: boolean): string {
    if (!ReturnKeyAction.performsEditorAction(action, disabled)) {
      return "换行";
    }
    switch (action) {
      case GO:
        return "前往";
      case SEARCH:
        return "搜索";
      case SEND:
        return "发送";
      case NEXT:
        return "下一项";
      case DONE:
        return "完成";
      case PREVIOUS:
        return "上一项";
      default:
        return "换行";
    }
  }
}
