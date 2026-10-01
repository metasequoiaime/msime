// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TextInputRow } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled row and forwards text input changes", () => {
  const onChange = vi.fn();
  render(
    <TextInputRow
      title="接口地址"
      label="翻译接口地址"
      value="https://example.test"
      placeholder="https://example.test/translate"
      autoComplete="off"
      onChange={onChange}
    />,
  );

  const input = screen.getByLabelText("翻译接口地址") as HTMLInputElement;
  expect(screen.getByText("接口地址")).toBeTruthy();
  expect(input.value).toBe("https://example.test");
  expect(input.placeholder).toBe("https://example.test/translate");
  expect(input.autocomplete).toBe("off");

  fireEvent.change(input, { target: { value: "https://updated.test" } });
  expect(onChange).toHaveBeenCalledWith("https://updated.test");
});

test("passes native input state through to the row control", () => {
  render(
    <TextInputRow
      title="凭据"
      label="服务凭据"
      value="synthetic-value"
      disabled
      spellCheck={false}
      onChange={vi.fn()}
    />,
  );

  const input = screen.getByLabelText("服务凭据") as HTMLInputElement;
  expect(input.disabled).toBe(true);
  expect(input.getAttribute("spellcheck")).toBe("false");
});
