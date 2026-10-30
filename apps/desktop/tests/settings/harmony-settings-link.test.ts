import { expect, test } from "vitest";
import {
  LINKED_PAGES,
  linkedPage,
  linkedTarget,
  pendingSettingsLink,
  settingsLink,
} from "../../../harmony/src/settings-link";
import { SettingsPageLink } from "../../../../platforms/harmony/entry/src/main/ets/keyboard/SettingsPageLink";

test("the page accepts the pages and targets the keyboard sends", () => {
  expect([...LINKED_PAGES].sort()).toEqual([...SettingsPageLink.PAGES].sort());
  for (const target of SettingsPageLink.targets("screen-keyboard")) {
    expect(settingsLink("screen-keyboard", target)).toEqual({ page: "screen-keyboard", target });
  }
});

test("a link with only a page is the old link", () => {
  expect(settingsLink("about")).toEqual({ page: "about" });
  expect(settingsLink("screen-keyboard", undefined)).toEqual({ page: "screen-keyboard" });
  expect(linkedPage("developer")).toBeUndefined();
  expect(settingsLink("developer", "touch_nine_key_sidebar_symbols")).toBeUndefined();
});

test("a target is kept only on its own page", () => {
  expect(linkedTarget("screen-keyboard", "touch_nine_key_digit_sidebar_symbols")).toBe(
    "touch_nine_key_digit_sidebar_symbols",
  );
  expect(settingsLink("about", "touch_nine_key_sidebar_symbols")).toEqual({ page: "about" });
  expect(settingsLink("screen-keyboard", "touch_number_row")).toEqual({ page: "screen-keyboard" });
  expect(settingsLink("screen-keyboard", ["touch_nine_key_sidebar_symbols"])).toEqual({
    page: "screen-keyboard",
  });
});

test("the host's pending reply carries the target beside the page", () => {
  expect(
    pendingSettingsLink(
      JSON.stringify({
        ok: true,
        value: "screen-keyboard",
        target: "touch_nine_key_sidebar_symbols",
      }),
    ),
  ).toEqual({ page: "screen-keyboard", target: "touch_nine_key_sidebar_symbols" });
  // 宿主没有待打开的页面时两项都是 null。
  expect(
    pendingSettingsLink(JSON.stringify({ ok: true, value: null, target: null })),
  ).toBeUndefined();
  // 旧宿主的回复没有 target。
  expect(pendingSettingsLink(JSON.stringify({ ok: true, value: "skin" }))).toEqual({
    page: "skin",
  });
  expect(pendingSettingsLink(JSON.stringify({ ok: false, value: "skin" }))).toBeUndefined();
  expect(pendingSettingsLink("not json")).toBeUndefined();
});
