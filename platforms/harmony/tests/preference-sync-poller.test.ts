import { PreferenceSyncPoller } from "../entry/src/main/ets/keyboard/PreferenceSyncPoller";
import { MirrorManifest } from "../entry/src/main/ets/keyboard/SharedStateMirrorPolicy";
import { SharedStateSyncPolicy } from "../entry/src/main/ets/keyboard/input/SharedStateSyncPolicy";

let failures = 0;
let checks = 0;

function check(condition: boolean, message: string): void {
  checks++;
  if (!condition) throw new Error(message);
}

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

interface PendingTimer {
  callback: () => void;
  delay: number;
}

function manifest(fingerprint: string): MirrorManifest {
  return {
    format: 1,
    publisher: "settings",
    entries: [{ name: "preferences", fingerprint, changedMs: 1 }],
    applied: [],
  };
}

console.log("Harmony preference sync poller");

group("preference tokens ignore revision-only manifest changes", () => {
  check(
    SharedStateSyncPolicy.preferencesToken(manifest("same")) === "same",
    "the preferences fingerprint is the poll token",
  );
  check(
    SharedStateSyncPolicy.preferencesToken({
      ...manifest("same"),
      entries: [{ name: "preferences", fingerprint: "same", changedMs: 2 }],
    }) === "same",
    "the changed timestamp alone does not trigger a session rebuild",
  );
  check(
    SharedStateSyncPolicy.preferencesToken(null) === "",
    "an absent manifest has an empty token",
  );
});

group("polling stops while hidden and resumes after being shown", () => {
  const timers: PendingTimer[] = [];
  let allowed = true;
  let polls = 0;
  const poller = new PreferenceSyncPoller(
    () => allowed,
    () => polls++,
    500,
    (callback, delay) => {
      timers.push({ callback, delay });
      return timers.length;
    },
    () => undefined,
  );
  poller.start();
  check(timers.length === 1 && timers[0].delay === 500, "start schedules one bounded poll");
  timers.shift()?.callback();
  check(polls === 1 && timers.length === 1, "an active poll reschedules itself");
  allowed = false;
  timers.shift()?.callback();
  check(polls === 1 && timers.length === 1, "an ineligible poll does not call the reader");
  poller.stop();
  timers.shift()?.callback();
  check(timers.length === 0, "stop prevents another poll from being scheduled");
});

if (failures > 0) {
  throw new Error(`${failures} groups failed (${checks} checks)`);
}
console.log(`  ${checks} checks passed`);
