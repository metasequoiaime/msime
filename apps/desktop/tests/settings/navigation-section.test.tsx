// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  NavigationSection,
  type NavigationPreferences,
  type WordCharacterPreferences,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const navigation: NavigationPreferences = {
  minus_equal: true,
  comma_period: true,
  brackets: false,
  tab: true,
  page_up_down: true,
  arrows: true,
};
const wordCharacter: WordCharacterPreferences = { enabled: true, keys: "brackets" };

test("enabling the active paging shortcut disables word-to-character", () => {
  const onChange = vi.fn();
  render(
    <NavigationSection
      navigation={navigation}
      wordCharacter={wordCharacter}
      linux={false}
      onChange={onChange}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "[ / ]" }));

  expect(onChange).toHaveBeenCalledWith({
    navigation: { ...navigation, brackets: true },
    wordCharacter: { enabled: false, keys: "brackets" },
  });
});

test("navigation section shows Linux paging guidance", () => {
  render(
    <NavigationSection
      navigation={navigation}
      wordCharacter={{ enabled: false, keys: "brackets" }}
      linux
      onChange={vi.fn()}
    />,
  );

  expect(screen.getByText(/IBus 候选窗口/)).toBeTruthy();
});

test("missing optional mouse wheel preference renders unchecked", () => {
  render(
    <NavigationSection
      navigation={navigation}
      wordCharacter={{ enabled: false, keys: "brackets" }}
      linux={false}
      onChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("checkbox", { name: "鼠标滚轮（候选窗口支持时翻页）" })).toHaveProperty(
    "checked",
    false,
  );
});

test("a host that fixes Tab and PageUp/PageDown paging lists neither and says they always page", () => {
  render(
    <NavigationSection
      navigation={{ ...navigation, tab: false, page_up_down: false }}
      wordCharacter={wordCharacter}
      linux={false}
      tabPageSwitches={false}
      onChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("checkbox", { name: "Shift+Tab / Tab" })).toBeNull();
  expect(screen.queryByRole("checkbox", { name: "PageUp / PageDown" })).toBeNull();
  expect(screen.getByRole("checkbox", { name: "- / =" })).toBeTruthy();
  expect(screen.getByText(/始终翻页/)).toBeTruthy();
});

test("other hosts keep the Tab and PageUp/PageDown switches", () => {
  render(
    <NavigationSection
      navigation={navigation}
      wordCharacter={wordCharacter}
      linux={false}
      onChange={vi.fn()}
    />,
  );
  expect(screen.getByRole("checkbox", { name: "Shift+Tab / Tab" })).toBeTruthy();
  expect(screen.getByRole("checkbox", { name: "PageUp / PageDown" })).toBeTruthy();
  expect(screen.queryByText(/始终翻页/)).toBeNull();
});
