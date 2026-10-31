import {
  CLIPBOARD_DELETE_AUTHENTICATION_ENTRY,
  CLIPBOARD_DELETE_AUTHENTICATION_FILE,
  CLIPBOARD_DELETE_AUTHENTICATION_MAX_BYTES,
  CLIPBOARD_DELETE_AUTHENTICATION_UNAVAILABLE_MESSAGE,
  ClipboardDeleteAuthenticationDecision,
  ClipboardDeleteAuthenticationPolicy,
  ClipboardDeleteGate,
} from "../entry/src/main/ets/keyboard/settings/ClipboardDeleteAuthenticationPolicy";
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

const decide = ClipboardDeleteAuthenticationPolicy.decide;
const D = ClipboardDeleteAuthenticationDecision;

// 与 Android 的 ClipboardDeleteAuthenticationPolicy 逐条对照（#10408）。
group("a value that does not change does nothing, with or without authentication", () => {
  for (const enabled of [false, true]) {
    for (const available of [false, true]) {
      check(
        decide(enabled, enabled, available) === D.UNCHANGED,
        `enabled=${enabled} available=${available}`,
      );
    }
  }
});

group("turning it on authenticates, the first time too", () => {
  check(decide(false, true, true) === D.AUTHENTICATE, "first switch-on");
});

group("turning it off authenticates", () => {
  check(decide(true, false, true) === D.AUTHENTICATE, "switch-off");
});

group("without system authentication the switch moves neither way", () => {
  check(
    decide(false, true, false) === D.UNAVAILABLE,
    "cannot turn on without a lock-screen credential",
  );
  check(
    decide(true, false, false) === D.UNAVAILABLE,
    "cannot turn off after the lock screen was removed",
  );
});

group("deletion is gated only while the switch is on", () => {
  const G = ClipboardDeleteGate;
  check(ClipboardDeleteAuthenticationPolicy.gate(false, true) === G.DELETE, "off deletes directly");
  check(
    ClipboardDeleteAuthenticationPolicy.gate(false, false) === G.DELETE,
    "off deletes directly without a lock screen",
  );
  check(
    ClipboardDeleteAuthenticationPolicy.gate(true, true) === G.AUTHENTICATE,
    "on authenticates first",
  );
  check(
    ClipboardDeleteAuthenticationPolicy.gate(true, false) === G.UNAVAILABLE,
    "on without a lock screen refuses",
  );
  check(
    CLIPBOARD_DELETE_AUTHENTICATION_UNAVAILABLE_MESSAGE.includes("锁屏"),
    "the refusal points to the lock screen",
  );
});

group("the file round-trips and a missing or broken one is off", () => {
  const parse = ClipboardDeleteAuthenticationPolicy.parse;
  for (const enabled of [false, true]) {
    check(
      parse(ClipboardDeleteAuthenticationPolicy.serialize(enabled)) === enabled,
      `round trip ${enabled}`,
    );
  }
  check(!parse(null), "no file is off");
  check(!parse(""), "an empty file is off");
  check(!parse("not json"), "garbage is off");
  check(!parse("[true]"), "an array is off");
  check(!parse("null"), "null is off");
  check(!parse('{"enabled":"true"}'), "a non-boolean is off");
  check(
    !parse(`{"enabled":true,"pad":"${"x".repeat(CLIPBOARD_DELETE_AUTHENTICATION_MAX_BYTES)}"}`),
    "an oversized file is off",
  );
  check(parse('{"enabled":true}'), "a stored opt-in is read back after a restart");
});

group("the keyboard receives the switch through the shared sandbox, one way", () => {
  const entry = SETTINGS_ENTRIES.find(
    (item) => item.name === CLIPBOARD_DELETE_AUTHENTICATION_ENTRY,
  );
  check(entry !== undefined, "settings publishes the switch");
  check(entry!.kind === "file" && entry!.base === "state", "as a file under files/state");
  check(entry!.path === CLIPBOARD_DELETE_AUTHENTICATION_FILE, "at the path both processes read");
  check(!entry!.fullModeOnly, "a keyboard in basic access mode still reads it");
  check(
    !KEYBOARD_ENTRIES.some((item) => item.name === CLIPBOARD_DELETE_AUTHENTICATION_ENTRY),
    "the keyboard never publishes it back, so it cannot switch the gate off",
  );
  check(
    SharedStateMirrorPolicy.importDecision(CLIPBOARD_DELETE_AUTHENTICATION_ENTRY) === "copy",
    "the keyboard copies it as is",
  );
  check(
    !SharedStateMirrorPolicy.keepsLocalWhenAbsent(CLIPBOARD_DELETE_AUTHENTICATION_ENTRY),
    "clearing the settings app's data turns the keyboard's copy off too",
  );
});

group("the switch is local and works without privacy consent", () => {
  for (const payload of ['{"operation":"load"}', '{"operation":"save","enabled":true}']) {
    check(
      !PrivacyConsentPolicy.refuses("clipboard_delete_authentication_settings", payload, false),
      `${payload} is not a network request`,
    );
  }
});

console.log(`\n${checks} checks, ${failures} failed`);
if (failures > 0) {
  throw new Error(`${failures} clipboard delete authentication group(s) failed`);
}
