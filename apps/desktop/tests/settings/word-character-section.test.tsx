// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { WordCharacterSection, type NavigationPreferences } from "@msime/ui";

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

function renderSection(
  wordCharacter = { enabled: true, keys: "brackets" as const },
  currentNavigation = navigation,
) {
  const onChange = vi.fn();
  render(
    <WordCharacterSection
      preferences={wordCharacter}
      navigation={currentNavigation}
      ios={false}
      onChange={onChange}
    />,
  );
  return onChange;
}

test("enabling word-to-character disables its selected paging key", () => {
  const onChange = renderSection({ enabled: false, keys: "brackets" });

  fireEvent.click(screen.getByRole("checkbox", { name: "以词定字" }));

  expect(onChange).toHaveBeenCalledWith({
    wordCharacter: { enabled: true, keys: "brackets" },
    navigation: { ...navigation, brackets: false },
  });
});

test("selecting a paging key updates word-to-character without changing navigation", () => {
  const onChange = renderSection(
    { enabled: true, keys: "brackets" },
    { ...navigation, minus_equal: false, brackets: true },
  );

  fireEvent.click(screen.getByRole("radio", { name: "- / =" }));

  expect(onChange).toHaveBeenCalledWith({
    wordCharacter: { enabled: true, keys: "minus_equal" },
    navigation: { ...navigation, minus_equal: false, brackets: true },
  });
});
