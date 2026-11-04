/**
 * 上屏后联想（#9433）在鸿蒙键盘上的规则：联想栏按键样式与 Android `PredictionButtonsPolicySmoke` 逐条相同，回车在联想时照常换行或执行编辑器动作，实体键盘的空格和数字在联想时交给运行时去选，2in1 候选窗在联想时出现，Esc 关闭联想，外部改动清掉旧联想。视图和会话是 ArkTS，接线由 `tests/run.sh` 钉住。
 */
import {
  NextWordPredictionPolicy,
  PredictionButtons,
} from "../entry/src/main/ets/keyboard/candidate/NextWordPredictionPolicy";
import {
  ReturnDispatch,
  ReturnKeyAction,
} from "../entry/src/main/ets/keyboard/input/ReturnKeyAction";
import { KeyAccessibilityPolicy } from "../entry/src/main/ets/keyboard/input/KeyAccessibilityPolicy";

let failures = 0;

function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

const P = NextWordPredictionPolicy;

group("the strip button style reads the shared preference leniently", () => {
  check(P.BUTTONS_PREFERENCE_KEY === "next_word_prediction_buttons", "shared preference key");
  // 写法与 client-core 的 NextWordPredictionButtons 一一对应。
  check(P.buttons("expand_only") === PredictionButtons.EXPAND_ONLY, "expand_only");
  check(P.buttons("close_and_expand") === PredictionButtons.CLOSE_AND_EXPAND, "close_and_expand");
  check(P.buttons("close_only") === PredictionButtons.CLOSE_ONLY, "close_only");
  check(P.buttons(undefined) === PredictionButtons.EXPAND_ONLY, "a missing key is today's strip");
  check(P.buttons("") === PredictionButtons.EXPAND_ONLY, "an empty value is today's strip");
  check(
    P.buttons("close_left") === PredictionButtons.EXPAND_ONLY,
    "a newer style reads as today's strip",
  );
});

group("the three styles place × and ∨ only while predicting", () => {
  check(
    !P.showsClose(PredictionButtons.EXPAND_ONLY, true) &&
      P.allowsExpand(PredictionButtons.EXPAND_ONLY, true),
    "expand only keeps today's strip",
  );
  check(
    P.showsClose(PredictionButtons.CLOSE_AND_EXPAND, true) &&
      P.allowsExpand(PredictionButtons.CLOSE_AND_EXPAND, true),
    "close and expand shows both",
  );
  check(
    P.showsClose(PredictionButtons.CLOSE_ONLY, true) &&
      !P.allowsExpand(PredictionButtons.CLOSE_ONLY, true),
    "close only drops the expand key",
  );
  for (const style of [
    PredictionButtons.EXPAND_ONLY,
    PredictionButtons.CLOSE_AND_EXPAND,
    PredictionButtons.CLOSE_ONLY,
  ]) {
    check(!P.showsClose(style, false), `no close key while composing: ${style}`);
    check(P.allowsExpand(style, false), `expand key unchanged while composing: ${style}`);
  }
});

group("a prediction is showing only when the view marks it and has candidates", () => {
  check(P.showing(true, 3), "marked with candidates");
  check(!P.showing(true, 0), "marked but empty");
  check(!P.showing(false, 3), "ordinary candidates");
  check(!P.showing(undefined, 3), "an ordinary view omits the field");
});

group("prediction candidates do not make Return confirm, but Space still selects", () => {
  check(P.composingCandidates(4, true) === 0, "Return has no composition to confirm");
  check(P.composingCandidates(4, false) === 4, "composition rows still count");
  // 联想时空格交给运行时选高亮的一条，读屏名称照普通候选读「选定」。
  check(KeyAccessibilityPolicy.space(4 > 0) === "选定", "the space key reads as selecting");
  check(KeyAccessibilityPolicy.closePrediction() === "关闭联想", "the × reads what it does");
});

group("Return during a prediction ends it and still acts on the editor", () => {
  // 以前按候选数分派，联想的候选让回车走 COMMIT_HIGHLIGHTED：引擎清掉建议却报未处理，换行或发送就丢了。
  check(
    ReturnKeyAction.dispatch(false, false, 5) === ReturnDispatch.COMMIT_HIGHLIGHTED,
    "candidates alone still mean a composition's highlighted row",
  );
  check(
    ReturnKeyAction.dispatch(false, false, 5, false, false, false, false, true) ===
      ReturnDispatch.FINISH_THEN_EDITOR,
    "a prediction finishes and then reaches the editor",
  );
  check(
    ReturnKeyAction.dispatch(false, false, 0, false, false, false, false, false) ===
      ReturnDispatch.EDITOR,
    "idle Return is unchanged",
  );
});

group("the 2in1 candidate window stays up for a prediction", () => {
  check(P.candidateWindowWanted(true, false), "a composition");
  check(
    P.candidateWindowWanted(false, true),
    "a prediction has no composition but needs the window",
  );
  check(!P.candidateWindowWanted(false, false), "idle");
});

group("Esc without modifiers dismisses a prediction", () => {
  check(P.dismissesOnKey(P.ESCAPE_KEYCODE, false, false, false, false), "plain Esc");
  check(
    !P.dismissesOnKey(P.ESCAPE_KEYCODE, true, false, false, false),
    "Ctrl+Esc is the application's",
  );
  check(
    !P.dismissesOnKey(P.ESCAPE_KEYCODE, false, false, false, true),
    "Shift+Esc is the application's",
  );
  check(!P.dismissesOnKey(2050, false, false, false, false), "Space selects, it does not dismiss");
  check(!P.dismissesOnKey(2054, false, false, false, false), "Enter is typed, not a dismissal");
});

group("physical Space and digits go to the runtime while predicting; Enter does not", () => {
  const select = (keyCode: number, unicodeChar: number, shift = false, ctrl = false, row = true) =>
    P.selectionKey(keyCode, unicodeChar, ctrl, false, false, shift, row);
  check(select(P.SPACE_KEYCODE, 0x20) === 0x20, "plain Space selects the highlighted row");
  check(select(2001, 0x31) === 0x31, "1 selects the first row on the page");
  check(select(2009, 0x39) === 0x39, "9 is the runtime's to judge against the page");
  check(select(2000, 0x30) === 0x30, "0 is the runtime's to judge too");
  check(
    select(2001, 0x31, false, false, false) === -1,
    "number-row selection off: digits are typed",
  );
  check(select(2001, 0x21, true) === -1, "Shift+1 is a mark, not a pick");
  check(select(P.SPACE_KEYCODE, 0x20, false, true) === -1, "Ctrl+Space is the application's");
  check(select(2054, 0x0d) === -1, "Enter still reaches the application");
  check(select(2055, 0) === -1, "Backspace still reaches the application");
  check(select(2017, 0x61) === -1, "a letter starts a new composition the usual way");
});

group("only an external editor change refocuses a prediction away", () => {
  check(P.refocusesOnEditorChange(false, true), "an external move or edit clears it");
  check(!P.refocusesOnEditorChange(true, true), "the keyboard's own commit echo keeps it");
  check(!P.refocusesOnEditorChange(false, false), "nothing to clear without a prediction");
});

if (failures > 0) {
  console.log(`${failures} next-word prediction group(s) failed`);
  throw new Error("next-word prediction checks failed");
}
