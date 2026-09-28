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
