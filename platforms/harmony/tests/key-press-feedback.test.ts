/**
 * 按键区按下即振动（#10757）：按下留令牌、上屏路径消耗、抬手后撤回，以及多指、快速连点和没有上屏的按下。规则与 Android `KeyPressFeedbackPolicySmoke` 相同，这里钉住鸿蒙这一份。
 */
import { KeyPressFeedback } from "../entry/src/main/ets/keyboard/input/KeyPressFeedback";

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

group("a tap vibrates on press and the commit stays silent", () => {
  const gate = new KeyPressFeedback();
  gate.down(0);
  const token = gate.up(0);
  check(gate.consume(), "the commit of a key that vibrated on press is silent");
  gate.settle(token);
  check(gate.armedCount() === 0, "nothing is left after the tap");
  check(!gate.consume(), "a later commit without a press (an accessibility click) vibrates");
});

group("a press without a commit does not swallow a later vibration", () => {
  const gate = new KeyPressFeedback();
  gate.down(0);
  gate.settle(gate.up(0));
  check(!gate.consume(), "Shift, a layer switch or a cancelled swipe leaves no token behind");
});

group("two fingers each silence their own commit", () => {
  const gate = new KeyPressFeedback();
  gate.down(0);
  gate.down(1);
  const first = gate.up(0);
  check(gate.consume(), "the first release is silent");
  gate.settle(first);
  check(gate.armedCount() === 1, "the finger still down keeps its token");
  const second = gate.up(1);
  check(gate.consume(), "the second release is silent too");
  gate.settle(second);
  check(!gate.consume(), "both fingers leave nothing armed");
});

group("a reused finger id is not disarmed by the previous tap's late settle", () => {
  const gate = new KeyPressFeedback();
  gate.down(0);
  const previous = gate.up(0);
  check(gate.consume(), "the first tap commits silently");
  gate.down(0);
  gate.settle(previous);
  check(gate.armedCount() === 1, "the second press keeps its token");
  const next = gate.up(0);
  check(gate.consume(), "the second tap commits silently");
  gate.settle(next);
  check(gate.armedCount() === 0, "nothing is left after two quick taps");
});

group("a press whose release never arrived is settled by the next press of that finger", () => {
  const gate = new KeyPressFeedback();
  gate.down(0);
  gate.down(0);
  check(gate.armedCount() === 1, "the lost press does not linger");
});

group("releasing a finger that never pressed the key area hands back nothing", () => {
  const gate = new KeyPressFeedback();
  check(gate.up(3) === undefined, "no token for an unknown finger");
  gate.settle(undefined);
  check(!gate.consume(), "nothing to consume");
});

group("clear drops every token", () => {
  const gate = new KeyPressFeedback();
  gate.down(0);
  gate.down(1);
  gate.clear();
  check(!gate.consume(), "no token survives clear");
  check(gate.up(0) === undefined, "no finger survives clear");
});

group("逆序松开的两根手指各自消耗本次按压", () => {
  const gate = new KeyPressFeedback();
  gate.down(10);
  gate.down(20);
  const second = gate.up(20);
  check(gate.consume(), "后按下的手指先上屏时不再振动");
  gate.settle(second);
  check(gate.armedCount() === 1, "后按下的手指延迟清理不能撤掉另一根手指");
  const first = gate.up(10);
  check(gate.consume(), "仍按着的第一根手指稍后上屏也不能重复振动");
  gate.settle(first);
  check(gate.armedCount() === 0, "两次上屏分别消费自己的令牌");
});

group("没有松手的按压不会被别的上屏偷走", () => {
  const gate = new KeyPressFeedback();
  gate.down(10);
  check(!gate.consume(), "另一处读屏或程序上屏不能消耗仍按着的手指");
  const token = gate.up(10);
  check(gate.consume(), "这根手指自己的松手仍可去重");
  gate.settle(token);
});

group("前一根未上屏的松手不占用下一根手指的上屏", () => {
  const gate = new KeyPressFeedback();
  gate.down(10);
  gate.down(20);
  const first = gate.up(10);
  const second = gate.up(20);
  check(gate.consume(), "后一次松手的点击消耗自己的令牌");
  gate.settle(second);
  check(gate.armedCount() === 1, "没有上屏的前一次按压仍只由自己的定时器清理");
  gate.settle(first);
  check(gate.armedCount() === 0, "清理完成没有残留");
});

declare function require(name: "fs"): { readFileSync(path: string, encoding: "utf8"): string };
declare function require(name: "module"): { stripTypeScriptTypes(source: string): string };

// 执行真实 ArkTS 方法，替换振动、计时器和手势结果，不再用源码字符串断言接线。
function methods(source: string, names: string[]): string {
  return names
    .map((name: string): string => {
      const match = source.match(
        new RegExp(`^  (?:private )?${name}\\([^\\n]*\\)[:\\s][\\s\\S]*?^  }`, "m"),
      );
      if (match === null) throw new Error(`没有找到真实方法 ${name}`);
      return match[0];
    })
    .join("\n");
}

const sessionSource = require("fs").readFileSync(
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets",
  "utf8",
);
const viewSource = require("fs").readFileSync(
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets",
  "utf8",
);
const touchTypes = { Down: 0, Up: 1, Move: 2, Cancel: 3 };

function adapter() {
  const timers: (() => void)[] = [];
  let vibrations = 0;
  const Session = new Function(
    "KeyPressFeedback",
    "setTimeout",
    require("module").stripTypeScriptTypes(
      `class Session {${methods(sessionSource, ["keyDown", "keyUp", "keyFeedback", "tapFeedback", "holdFeedback"])}};`,
    ) + "return Session;",
  )(KeyPressFeedback, (callback: () => void): number => timers.push(callback));
  const session = new Session();
  session.keyPress = new KeyPressFeedback();
  session.vibrateKey = (): void => {
    vibrations += 1;
  };
  session.softKeyPress = (): void => {};
  session.finishComposition = (): void => {};
  session.insert = (): void => {};
  const View = new Function(
    "KeyboardSession",
    "TouchType",
    "NineKeyFlick",
    "NineKeyFlickPolicy",
    "NineKeyLayout",
    "KeyIdPolicy",
    "GlideTypingPolicy",
    require("module").stripTypeScriptTypes(
      `class View {${methods(viewSource, [
        "keyPressTouch",
        "nineKeyFlickTouch",
        "typeNineKeyFlickCharacter",
        "eighteenKeySwipeTouch",
        "commaSwipeTouch",
        "letterHintTouch",
        "glideUp",
      ])} };`,
    ) + "return View;",
  )(
    { shared: session },
    touchTypes,
    { FIRST: 1, SECOND: 2, THIRD: 3, NONE: 0 },
    { character: (): string => "a" },
    { digitHint: (): string => "2" },
    { nineKey: (): string => "synthetic-key", character: (): string => "synthetic-key" },
    { request: (): string => "synthetic-glide" },
  );
  const view = new View();
  view.keyFacesShowKeys = (): boolean => true;
  view.nineKeyFlicks = { up: (): number => 1 };
  const event = (type: number, id: number) => ({ type, changedTouches: [{ id }] });
  return { session, view, event, timers, vibrations: (): number => vibrations };
}

group("真实视图和会话在逆序多指松手后不重复振动", () => {
  const f = adapter();
  f.view.keyPressTouch(f.event(touchTypes.Down, 10));
  f.view.keyPressTouch(f.event(touchTypes.Down, 20));
  check(f.vibrations() === 2, "每根手指按下各振一次");
  f.view.keyPressTouch(f.event(touchTypes.Up, 20));
  f.session.keyFeedback();
  f.timers.shift()!();
  f.view.keyPressTouch(f.event(touchTypes.Up, 10));
  f.session.keyFeedback();
  f.timers.shift()!();
  check(f.vibrations() === 2, "先清理 B 再松开 A 也不能增加第三次振动");
  f.session.holdFeedback();
  check(f.vibrations() === 3, "长按确认仍独立反馈");
});

group("九键子控件松手上屏在容器收到抬起之前绑定自己的按压", () => {
  const f = adapter();
  f.view.keyPressTouch(f.event(touchTypes.Down, 10));
  f.view.keyPressTouch(f.event(touchTypes.Down, 20));
  const up = f.event(touchTypes.Up, 20);
  f.view.nineKeyFlickTouch({ input: "abc" }, up);
  f.view.keyPressTouch(up);
  while (f.timers.length > 0) f.timers.shift()!();
  f.view.keyPressTouch(f.event(touchTypes.Up, 10));
  f.session.keyFeedback();
  check(f.vibrations() === 2, "滑数字和另一根手指的点击都各自去重");
});

group("其他子控件松手上屏也绑定自己的按压", () => {
  for (const gesture of ["eighteen", "comma", "letter", "glide"]) {
    const f = adapter();
    f.view.keyPressTouch(f.event(touchTypes.Down, 10));
    f.view.keyPressTouch(f.event(touchTypes.Down, 20));
    const up = f.event(touchTypes.Up, 20);
    f.view.swipesEighteenKey = (): boolean => true;
    f.view.eighteenSwipes = { up: (): string => "A" };
    f.view.endEighteenSwipeFace = (): void => {};
    f.view.typeEighteenKeyLetter = (): void => f.session.keyFeedback();
    f.view.commaSwipeEnabled = (): boolean => true;
    f.view.commaSwipe = { up: (): boolean => true };
    f.session.punctuation = (): void => f.session.keyFeedback();
    f.view.cancelHintHold = (): void => {};
    f.view.hideKeyPreview = (): void => {};
    f.view.helpcodeSwipes = { up: (): string => "" };
    f.view.letterHints = { up: (): string => "synthetic-hint" };
    f.view.typeLetterHint = (): void => f.session.keyFeedback();
    f.view.glideActive = gesture === "glide";
    f.view.glideFinger = 20;
    f.view.changedTouch = (): { id: number } => ({ id: 20 });
    f.view.addGlidePoint = (): void => {};
    f.view.glideReset = (): void => {};
    f.session.glide = (): void => f.session.keyFeedback();
    if (gesture === "eighteen") f.view.eighteenKeySwipeTouch({ letters: "AB" }, up);
    else if (gesture === "comma") f.view.commaSwipeTouch(up);
    else if (gesture === "letter") f.view.letterHintTouch("a", "synthetic-hint", up);
    else f.view.glideUp(up);
    f.view.keyPressTouch(up);
    while (f.timers.length > 0) f.timers.shift()!();
    f.view.keyPressTouch(f.event(touchTypes.Up, 10));
    f.session.keyFeedback();
    check(f.vibrations() === 2, `${gesture} 松手不重复振动，也不偷另一根手指的令牌`);
  }
});

group("真实拼写列表触摸跳过反馈且保留冒泡", () => {
  const f = adapter();
  const grid = viewSource.slice(viewSource.indexOf("  nineKeyGrid() {"));
  const match = grid.match(/\.onTouch\(\(event: TouchEvent\): void => \{([\s\S]*?)^          }\)/m);
  check(match !== null, "没有找到拼写列表的触摸回调");
  const callback = new Function(
    "TouchType",
    require("module").stripTypeScriptTypes(
      `function callback(event: TouchEvent): void {${match![1]}};`,
    ) + "return callback;",
  )(touchTypes);
  const down = f.event(touchTypes.Down, 10);
  callback.call(f.view, down);
  f.view.keyPressTouch(down);
  check(f.vibrations() === 0, "滚动或点选拼写不振动");
  f.view.keyPressTouch(f.event(touchTypes.Down, 20));
  check(f.vibrations() === 1, "拼写列表不影响后续正常按键");
});

if (failures > 0) {
  console.log(`${failures} key press feedback group(s) failed`);
  throw new Error("key press feedback checks failed");
}
