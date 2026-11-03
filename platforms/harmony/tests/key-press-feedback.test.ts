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

if (failures > 0) {
  console.log(`${failures} key press feedback group(s) failed`);
  throw new Error("key press feedback checks failed");
}
