import {
  SCREEN_CAPTURE_PROTECTION_ENTRY,
  SCREEN_CAPTURE_PROTECTION_FILE,
  ScreenCaptureProtectionDecision,
  ScreenCaptureProtectionPolicy,
  ScreenCaptureProtectionState,
} from "../entry/src/main/ets/keyboard/settings/ScreenCaptureProtectionPolicy";
import {
  KEYBOARD_ENTRIES,
  SETTINGS_ENTRIES,
  SharedStateMirrorPolicy,
} from "../entry/src/main/ets/keyboard/SharedStateMirrorPolicy";
import { PrivacyConsentPolicy } from "../entry/src/main/ets/privacy/PrivacyConsentPolicy";

let failures = 0;
let checks = 0;

/** 放在本地定义，这样测试套件不需要 node 类型定义，本仓库也没有这些定义。 */
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
  checks++;
  if (!condition) {
    throw new Error(message);
  }
}

function state(enabled: boolean, enabledBefore: boolean): ScreenCaptureProtectionState {
  return { enabled, enabledBefore };
}

const decide = ScreenCaptureProtectionPolicy.decide;
const D = ScreenCaptureProtectionDecision;

// 与 Android 的 ScreenCaptureProtectionPolicy.decide 逐条对照（#10159）。
group("a value that does not change does nothing, with or without authentication", () => {
  for (const enabled of [false, true]) {
    for (const before of [false, true]) {
      for (const available of [false, true]) {
        check(
          decide(state(enabled, before), enabled, available) === D.UNCHANGED,
          `enabled=${enabled} before=${before} available=${available}`,
        );
      }
    }
  }
});

group("the first switch-on saves without authentication", () => {
  check(decide(state(false, false), true, true) === D.SAVE, "never turned on before");
});

group("once turned on, turning it on or off again needs authentication", () => {
  check(decide(state(true, true), false, true) === D.AUTHENTICATE, "turning off");
  check(decide(state(false, true), true, true) === D.AUTHENTICATE, "turning on again");
});

group("without system authentication nothing changes, not even the first switch-on", () => {
  check(decide(state(false, false), true, false) === D.UNAVAILABLE, "first switch-on refused");
  check(decide(state(true, true), false, false) === D.UNAVAILABLE, "switch-off refused");
  check(decide(state(false, true), true, false) === D.UNAVAILABLE, "switch-on again refused");
});

group("on without the turned-on mark counts as turned on before", () => {
  check(
    decide(state(true, false), false, true) === D.AUTHENTICATE,
    "switch-off still authenticates",
  );
  const parsed = ScreenCaptureProtectionPolicy.parse('{"enabled":true}');
  check(
    parsed.enabled && parsed.enabledBefore,
    "a file that is on but lacks the mark reads as turned on before",
  );
});

group("saving remembers that it was turned on", () => {
  const first = ScreenCaptureProtectionPolicy.after(state(false, false), true);
  check(first.enabled && first.enabledBefore, "the first switch-on writes the mark with it");
  const off = ScreenCaptureProtectionPolicy.after(first, false);
  check(!off.enabled && off.enabledBefore, "switching off keeps the mark");
  check(decide(off, true, true) === D.AUTHENTICATE, "so switching on again authenticates");
});

group("the file round-trips and a missing or broken one is off and never turned on", () => {
  for (const value of [state(false, false), state(true, true), state(false, true)]) {
    const read = ScreenCaptureProtectionPolicy.parse(
      ScreenCaptureProtectionPolicy.serialize(value),
    );
    check(
      read.enabled === value.enabled && read.enabledBefore === value.enabledBefore,
      `${JSON.stringify(value)} survives a write and a read`,
    );
  }
  const written = JSON.parse(ScreenCaptureProtectionPolicy.serialize(state(true, false))) as {
    enabled: boolean;
    enabled_before: boolean;
  };
  check(written.enabled && written.enabled_before, "the file never holds on without the mark");
  for (const text of [
    null,
    "",
    "not json",
    "null",
    "[]",
    '"on"',
    '{"enabled":"true"}',
    "x".repeat(5000),
  ]) {
    const read = ScreenCaptureProtectionPolicy.parse(text);
    check(!read.enabled && !read.enabledBefore, `${String(text).slice(0, 20)} reads as off`);
  }
  check(
    ScreenCaptureProtectionPolicy.parse('{"enabled":false,"enabled_before":true}').enabledBefore,
    "off with the mark keeps the mark",
  );
});

group("the keyboard receives the switch through the shared sandbox, one way", () => {
  const entry = SETTINGS_ENTRIES.find((item) => item.name === SCREEN_CAPTURE_PROTECTION_ENTRY);
  check(entry !== undefined, "settings publishes the switch");
  check(entry!.kind === "file" && entry!.base === "state", "as a file under files/state");
  check(entry!.path === SCREEN_CAPTURE_PROTECTION_FILE, "at the path both processes read");
  check(!entry!.fullModeOnly, "a keyboard in basic access mode still reads it");
  check(
    !KEYBOARD_ENTRIES.some((item) => item.name === SCREEN_CAPTURE_PROTECTION_ENTRY),
    "the keyboard never publishes it back, so it cannot switch protection off",
  );
  check(
    SharedStateMirrorPolicy.importDecision(SCREEN_CAPTURE_PROTECTION_ENTRY) === "copy",
    "the keyboard copies it as is",
  );
  check(
    !SharedStateMirrorPolicy.keepsLocalWhenAbsent(SCREEN_CAPTURE_PROTECTION_ENTRY),
    "a settings app without the file (data cleared) turns it off in the keyboard too",
  );
});

group("the switch is local and works without privacy consent", () => {
  for (const payload of ['{"operation":"load"}', '{"operation":"save","enabled":true}']) {
    check(
      !PrivacyConsentPolicy.refuses("screen_capture_protection_settings", payload, false),
      `${payload} is not a network request`,
    );
  }
});

console.log(`\n${checks} checks, ${failures} failed`);
if (failures > 0) {
  throw new Error(`${failures} screen capture protection group(s) failed`);
}
