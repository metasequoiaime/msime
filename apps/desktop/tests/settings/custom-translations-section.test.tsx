// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CustomTranslationsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards edits and save requests", () => {
  const onChange = vi.fn();
  const onSave = vi.fn();
  render(
    <CustomTranslationsSection
      mobile={false}
      value="hello\t你好"
      placeholder="example"
      notice=""
      summary="1 条释义"
      busy={false}
      onChange={onChange}
      onSave={onSave}
    />,
  );

  fireEvent.change(screen.getByLabelText("自定义候选释义"), {
    target: { value: "world\t世界" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存自定义释义" }));

  expect(onChange).toHaveBeenCalledWith("world\t世界");
  expect(onSave).toHaveBeenCalledOnce();
  expect(screen.getByText(/候选窗/)).toBeTruthy();
});

test("shows the notice and disables save while busy", () => {
  render(
    <CustomTranslationsSection
      mobile
      value=""
      placeholder="example"
      notice="保存失败"
      summary="0 条释义"
      busy
      onChange={vi.fn()}
      onSave={vi.fn()}
    />,
  );

  expect(screen.getByRole("status").textContent).toContain("保存失败");
  expect((screen.getByRole("button", { name: "保存中…" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect(screen.getByText(/候选栏/)).toBeTruthy();
});
