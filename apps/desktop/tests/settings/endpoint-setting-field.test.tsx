// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { EndpointSettingField } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("labels a URL field and forwards changes", () => {
  const onChange = vi.fn();
  render(
    <EndpointSettingField
      label="接口地址"
      inputLabel="AI 接口地址"
      description="留空使用默认地址"
      value="https://synthetic.example.test/v1"
      onChange={onChange}
    />,
  );

  const input = screen.getByRole("textbox", { name: "AI 接口地址" }) as HTMLInputElement;
  expect(screen.getByText("接口地址")).toBeTruthy();
  expect(screen.getByText("留空使用默认地址")).toBeTruthy();
  expect(input.type).toBe("url");
  expect(input.value).toBe("https://synthetic.example.test/v1");

  fireEvent.change(input, { target: { value: "https://other.example.test/v2" } });
  expect(onChange).toHaveBeenCalledWith("https://other.example.test/v2");
});

test("forwards disabled and placeholder to the URL input", () => {
  render(
    <EndpointSettingField
      label="地址"
      inputLabel="测试接口地址"
      value=""
      disabled
      placeholder="https://synthetic.example.test"
      onChange={vi.fn()}
    />,
  );

  const input = screen.getByRole("textbox", { name: "测试接口地址" }) as HTMLInputElement;
  expect(input.disabled).toBe(true);
  expect(input.placeholder).toBe("https://synthetic.example.test");
});
