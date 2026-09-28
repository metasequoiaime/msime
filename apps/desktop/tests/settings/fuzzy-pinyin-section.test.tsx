// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { FuzzyPinyinSection, type FuzzyPinyinPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const basePreferences: FuzzyPinyinPreferences = { enabled: false, rules: [] };

function renderSection(
  preferences: FuzzyPinyinPreferences = basePreferences,
  onChange = vi.fn(),
  confirm = vi.fn().mockResolvedValue(true),
) {
  render(<FuzzyPinyinSection preferences={preferences} onChange={onChange} confirm={confirm} />);
  return { onChange, confirm };
}

test("first enable seeds every fuzzy-pinyin rule", () => {
  const { onChange } = renderSection();

  fireEvent.click(screen.getByRole("checkbox", { name: "启用模糊音" }));

  expect(onChange).toHaveBeenCalledWith({
    enabled: true,
    rules: [
      "z-zh",
      "c-ch",
      "s-sh",
      "n-l",
      "f-h",
      "r-l",
      "an-ang",
      "en-eng",
      "in-ing",
      "ian-iang",
      "uan-uang",
    ],
    seeded: true,
  });
});

test("reset asks for confirmation before clearing fuzzy-pinyin rules", async () => {
  const { onChange, confirm } = renderSection({
    enabled: true,
    rules: ["z-zh"],
    seeded: true,
  });

  fireEvent.click(screen.getByRole("button", { name: "重置模糊音配置" }));

  await waitFor(() => expect(confirm).toHaveBeenCalled());
  expect(onChange).toHaveBeenCalledWith({ enabled: false, rules: [], seeded: true });
});
