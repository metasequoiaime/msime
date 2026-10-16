// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DoubaoStreamEndpointSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("selects a preset endpoint and forwards its URL", () => {
  const onChange = vi.fn();
  render(<DoubaoStreamEndpointSection endpoint="" onChange={onChange} />);

  const select = screen.getByRole("combobox", { name: "流式接口" }) as HTMLSelectElement;
  // 空地址就是豆包的默认地址（双向流式）。
  expect(select.value).toBe("async");
  fireEvent.change(select, { target: { value: "nostream" } });
  expect(onChange).toHaveBeenCalledWith(expect.stringContaining("bigmodel_nostream"));
});

test("keeps a custom endpoint represented as custom", () => {
  render(
    <DoubaoStreamEndpointSection endpoint="https://custom.example/stream" onChange={vi.fn()} />,
  );
  expect((screen.getByRole("combobox", { name: "流式接口" }) as HTMLSelectElement).value).toBe(
    "custom",
  );
  // 自定义地址和预设是同一个控件：不是预设时，输入框就在下拉下面。
  expect((screen.getByRole("textbox", { name: "识别接口地址" }) as HTMLInputElement).value).toBe(
    "https://custom.example/stream",
  );
});
