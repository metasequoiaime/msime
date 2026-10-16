import { expect, test } from "vitest";
import { createMobileSettingsHistoryState } from "../../../../packages/ui/src/settings/mobile-history-state";

test("保留宿主字段并用本次路由覆盖旧路由", () => {
  const host = { source: "synthetic-host" };
  const current = Object.freeze({ host, page: "input", panel: "voice", msimeSettings: false });
  const patch = Object.freeze({ page: "account", tab: "account" });
  const state = createMobileSettingsHistoryState(current, patch);
  expect(state).toEqual({
    host,
    page: "account",
    panel: "voice",
    tab: "account",
    msimeSettings: true,
  });
  expect(state).not.toBe(current);
  expect(state.host).toBe(host);
  expect(current).toEqual({ host, page: "input", panel: "voice", msimeSettings: false });
  expect(patch).toEqual({ page: "account", tab: "account" });
});

test.each([undefined, null, false, 0, "synthetic-host"])(
  "旧状态 %s 不是对象时只使用新路由",
  (current) => {
    expect(createMobileSettingsHistoryState(current, { page: "input" })).toEqual({
      msimeSettings: true,
      page: "input",
    });
  },
);

test("数组旧状态保留自身字段", () => {
  expect(createMobileSettingsHistoryState(["synthetic-host"], { panel: "voice" })).toEqual({
    0: "synthetic-host",
    msimeSettings: true,
    panel: "voice",
  });
});

test("调用方字段最后覆盖默认标记", () => {
  expect(
    createMobileSettingsHistoryState({ msimeSettings: true }, { msimeSettings: false }),
  ).toEqual({
    msimeSettings: false,
  });
});
