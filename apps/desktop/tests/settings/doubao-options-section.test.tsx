// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DoubaoOptionsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards recognition option changes", () => {
  const onEnableItnChange = vi.fn();
  const onEnablePuncChange = vi.fn();
  const onEnableDdcChange = vi.fn();
  const onBoostingTableIdChange = vi.fn();
  render(
    <DoubaoOptionsSection
      linux={false}
      enableItn
      enablePunc
      enableDdc={false}
      boostingTableId="synthetic-table"
      onEnableItnChange={onEnableItnChange}
      onEnablePuncChange={onEnablePuncChange}
      onEnableDdcChange={onEnableDdcChange}
      onBoostingTableIdChange={onBoostingTableIdChange}
    />,
  );

  fireEvent.click(screen.getByLabelText("数字格式化"));
  fireEvent.click(screen.getByLabelText("标点预测"));
  fireEvent.click(screen.getByLabelText("语义顺滑"));
  fireEvent.change(screen.getByLabelText("热词表 ID"), {
    target: { value: "updated-table" },
  });

  expect(onEnableItnChange).toHaveBeenCalledWith(false);
  expect(onEnablePuncChange).toHaveBeenCalledWith(false);
  expect(onEnableDdcChange).toHaveBeenCalledWith(true);
  expect(onBoostingTableIdChange).toHaveBeenCalledWith("updated-table");
});

test("describes provider-side options", () => {
  render(
    <DoubaoOptionsSection
      linux
      enableItn
      enablePunc
      enableDdc={false}
      boostingTableId=""
      onEnableItnChange={vi.fn()}
      onEnablePuncChange={vi.fn()}
      onEnableDdcChange={vi.fn()}
      onBoostingTableIdChange={vi.fn()}
    />,
  );

  expect(screen.getByText("由 provider 服务应用")).toBeTruthy();
});
