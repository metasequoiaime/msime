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
  wordCharacter: { enabled: boolean; keys: "brackets" | "minus_equal" } = {
    enabled: true,
    keys: "brackets",
  },
  currentNavigation = navigation,
  ios = false,
) {
  const onChange = vi.fn();
  render(
    <WordCharacterSection
      preferences={wordCharacter}
      navigation={currentNavigation}
      ios={ios}
      onChange={onChange}
    />,
  );
  return onChange;
}

test("one choice covers off and both key groups, and 关闭 keeps the stored keys", () => {
  const onChange = renderSection(
    { enabled: true, keys: "minus_equal" },
    {
      ...navigation,
      minus_equal: false,
    },
  );

  expect(screen.queryByRole("switch", { name: "以词定字" })).toBeNull();
  expect((screen.getByRole("radio", { name: "- / =" }) as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: "关闭" }));
  // 关掉时记着的键组不动，下次打开还是它。
  expect(onChange).toHaveBeenLastCalledWith({
    wordCharacter: { enabled: false, keys: "minus_equal" },
    navigation: { ...navigation, minus_equal: false },
  });
});

test("a key group held by paging cannot be chosen, and choosing a free one turns it on", () => {
  // 缺省翻页占着 - / =，所以只能选 [ / ]。
  const onChange = renderSection({ enabled: false, keys: "minus_equal" });

  expect((screen.getByRole("radio", { name: "关闭" }) as HTMLInputElement).checked).toBe(true);
  expect((screen.getByRole("radio", { name: "- / =" }) as HTMLInputElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: "[ / ]" }));
  expect(onChange).toHaveBeenLastCalledWith({
    wordCharacter: { enabled: true, keys: "brackets" },
    navigation,
  });
});

test("iOS keeps the long-press switch", () => {
  const onChange = renderSection({ enabled: false, keys: "brackets" }, navigation, true);

  fireEvent.click(screen.getByRole("switch", { name: "以词定字" }));

  expect(onChange).toHaveBeenCalledWith({
    wordCharacter: { enabled: true, keys: "brackets" },
    navigation: { ...navigation, brackets: false },
  });
  expect(screen.queryByRole("radio", { name: "关闭" })).toBeNull();
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
