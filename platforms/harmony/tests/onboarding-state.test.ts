import {
  ImeEnabledState,
  OnboardingStatePolicy,
  SetupState,
} from "../entry/src/main/ets/keyboard/input/OnboardingStatePolicy";

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

function same(actual: SetupState, enabled: boolean | null, current: boolean | null): boolean {
  return actual.enabled === enabled && actual.current === current;
}

const own = "app.msime.hmos";

console.log("OnboardingStatePolicy setup facts");

group("the setup rows read the two facts separately", () => {
  check(
    same(
      OnboardingStatePolicy.setup({
        enabled: ImeEnabledState.DISABLED,
        currentBundle: "com.example.other",
        ownBundle: own,
      }),
      false,
      false,
    ),
    "a disabled keyboard is neither on nor current",
  );
  check(
    same(
      OnboardingStatePolicy.setup({
        enabled: ImeEnabledState.BASIC_MODE,
        currentBundle: own,
        ownBundle: own,
      }),
      true,
      true,
    ),
    "basic mode counts as enabled",
  );
  check(
    same(
      OnboardingStatePolicy.setup({
        enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
        currentBundle: "com.example.other",
        ownBundle: own,
      }),
      true,
      false,
    ),
    "enabled but another keyboard is in use",
  );
});

group("an unreadable answer is unknown, not no", () => {
  check(
    same(
      OnboardingStatePolicy.setup({ enabled: null, currentBundle: own, ownBundle: own }),
      null,
      true,
    ),
    "an unreadable enablement state does not hide that this keyboard is current",
  );
  check(
    same(
      OnboardingStatePolicy.setup({
        enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
        currentBundle: "",
        ownBundle: own,
      }),
      true,
      null,
    ),
    "an unreadable current keyboard is unknown",
  );
  // 否则读不出的自身名称会与每个键盘都比较为不相等，包括它自己。
  check(
    same(
      OnboardingStatePolicy.setup({
        enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
        currentBundle: own,
        ownBundle: "",
      }),
      true,
      null,
    ),
    "an unreadable own bundle is unknown",
  );
});

group("the welcome flow is decided from the two facts alone", () => {
  check(
    OnboardingStatePolicy.requiredFor({ enabled: false, current: null }) === true,
    "not enabled opens the flow even when current is unknown",
  );
  check(
    OnboardingStatePolicy.requiredFor({ enabled: true, current: false }) === true,
    "enabled but not current still has a step left",
  );
  check(
    OnboardingStatePolicy.requiredFor({ enabled: true, current: true }) === false,
    "enabled and current opens settings",
  );
  check(
    OnboardingStatePolicy.requiredFor({ enabled: null, current: false }) === false,
    "unknown enablement opens settings",
  );
  check(
    OnboardingStatePolicy.requiredFor({ enabled: true, current: null }) === false,
    "unknown current keyboard opens settings",
  );
});

group("the split agrees with the original decision for every combination", () => {
  const states: Array<ImeEnabledState | null> = [
    null,
    ImeEnabledState.DISABLED,
    ImeEnabledState.BASIC_MODE,
    ImeEnabledState.FULL_EXPERIENCE_MODE,
  ];
  const bundles: string[] = ["", own, "com.example.other"];
  for (const enabled of states) {
    for (const currentBundle of bundles) {
      for (const ownBundle of ["", own]) {
        const state = { enabled, currentBundle, ownBundle };
        const legacy =
          enabled === null
            ? false
            : enabled === ImeEnabledState.DISABLED
              ? true
              : currentBundle.length === 0 || ownBundle.length === 0
                ? false
                : currentBundle !== ownBundle;
        check(
          OnboardingStatePolicy.required(state) === legacy,
          `required(${JSON.stringify(state)}) changed meaning`,
        );
        check(
          OnboardingStatePolicy.requiredFor(OnboardingStatePolicy.setup(state)) === legacy,
          `requiredFor(setup(${JSON.stringify(state)})) changed meaning`,
        );
      }
    }
  }
});

console.log("");
if (failures > 0) {
  throw new Error(`${failures} group(s) failed`);
}
console.log(`all groups passed (${checks} assertions)`);
