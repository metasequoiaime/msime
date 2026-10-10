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

group("an unsigned or unconfigured build reports the fallback explicitly", () => {
  const status = HarmonySharedSandboxPolicy.status();
  check(status.kind === "fallback", "empty group id uses the fallback state directory");
  check(
    status.reason === "data-group-id-unconfigured",
    "the reason identifies the missing capability",
  );
  check(!status.available, "the fallback is not reported as shared");
});

group("the current package cannot claim a group id without a signed profile", () => {
  const status = HarmonySharedSandboxPolicy.status();
  check(status.kind === "fallback", "the package uses the fallback state directory");
  check(status.reason === "data-group-id-unconfigured", "the reason is stable for the UI");
  check(status.available === false, "sharing is never claimed by the unsigned package");
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} shared sandbox policy checks failed`);
