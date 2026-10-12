import { HarmonySharedSandboxPolicy } from "../entry/src/main/ets/keyboard/HarmonySharedSandboxPolicy";

let failures = 0;
let checks = 0;

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
  if (!condition) throw new Error(message);
}

console.log("Harmony shared sandbox capability policy");

group("no group directory falls back to the private state directory", () => {
  for (const directory of [null, ""]) {
    const status = HarmonySharedSandboxPolicy.status(directory);
    check(!status.available, "a missing group directory is never reported as shared");
    check(status.directory === "", "the fallback carries no directory");
    check(status.reason === "group-directory-unavailable", "the reason is stable for the log");
  }
});

group("a group directory from getGroupDir is shared", () => {
  const status = HarmonySharedSandboxPolicy.status("/data/storage/el2/group/synthetic");
  check(status.available, "the directory is reported as shared");
  check(
    status.directory === "/data/storage/el2/group/synthetic",
    "the directory is passed through",
  );
  check(status.reason === "", "an available sandbox has no reason");
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} shared sandbox policy checks failed`);
