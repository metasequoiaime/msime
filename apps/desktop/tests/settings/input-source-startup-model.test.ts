import { expect, test } from "vitest";
import {
  inputSourceNeedsAdding,
  inputSourceNeedsRecheck,
  inputSourceNoticeVisible,
  type InputSourceStartupStatus,
} from "../../../../packages/ui/src/settings/input-source-startup-model";

const actions: InputSourceStartupStatus["action"][] = [
  "installed",
  "updated",
  "up_to_date",
  "not_installed",
  "login_required",
  "authorization_required",
  "cancelled",
  "failed",
];

const outcomes = actions.flatMap((action) =>
  [true, false, null].flatMap((enabled) =>
    [false, true].map((duplicate) => ({ action, enabled, duplicate })),
  ),
);

test.each(outcomes)(
  "$action enabled=$enabled duplicate=$duplicate 保留添加、重查和提示条件",
  ({ action, enabled, duplicate }) => {
    const status: InputSourceStartupStatus = Object.freeze({
      action,
      enabled,
      bundled_version: "1.0.0",
      installed_version: "1.0.0",
      system_bundles: duplicate ? ["/synthetic/input-source.app"] : [],
    });
    const adding = ["installed", "updated", "up_to_date"].includes(action) && enabled === false;
    expect(inputSourceNeedsAdding(status)).toBe(adding);
    expect(inputSourceNeedsRecheck(status)).toBe(adding || duplicate);
    expect(inputSourceNoticeVisible(status)).toBe(
      action !== "up_to_date" || enabled === false || duplicate,
    );
  },
);

test("没有启动结果时不显示、不重查也不要求添加", () => {
  expect(inputSourceNeedsAdding(null)).toBe(false);
  expect(inputSourceNeedsRecheck(null)).toBe(false);
  expect(inputSourceNoticeVisible(null)).toBe(false);
});

test("没有系统副本字段的旧宿主仍按添加状态判定", () => {
  const status: InputSourceStartupStatus = {
    action: "up_to_date",
    enabled: true,
    bundled_version: null,
    installed_version: null,
  };
  expect(inputSourceNeedsRecheck(status)).toBe(false);
  expect(inputSourceNoticeVisible(status)).toBe(false);
});
