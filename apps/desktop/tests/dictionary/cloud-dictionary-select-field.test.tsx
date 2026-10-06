// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CloudDictionarySelectField } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled cloud select and forwards changes", () => {
  const onChange = vi.fn();
  render(
    <CloudDictionarySelectField
      label="编码方案"
      ariaLabel="云端编码方案"
      value="pinyin"
      labelClassName="field-label"
      selectClassName="field-select"
      onChange={onChange}
    >
      <option value="pinyin">全拼</option>
      <option value="shuangpin">双拼</option>
    </CloudDictionarySelectField>,
  );

  const select = screen.getByRole("combobox", { name: "云端编码方案" }) as HTMLSelectElement;
  expect(screen.getByText("编码方案")).toBeTruthy();
  expect(select.value).toBe("pinyin");
  expect(select.className).toBe("field-select");
  expect(select.closest("label")?.className).toBe("field-label");

  fireEvent.change(select, { target: { value: "shuangpin" } });
  expect(onChange).toHaveBeenCalledWith("shuangpin");
});

test("forwards disabled state", () => {
  render(
    <CloudDictionarySelectField
      label="格式"
      ariaLabel="云端格式"
      value="hans"
      disabled
      onChange={vi.fn()}
    >
      <option value="hans">汉字</option>
    </CloudDictionarySelectField>,
  );

  expect((screen.getByRole("combobox", { name: "云端格式" }) as HTMLSelectElement).disabled).toBe(
    true,
  );
});
