// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CustomTranslationsSection, type CustomTranslationsSectionProps } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function renderSection(overrides: Partial<CustomTranslationsSectionProps> = {}) {
  const props: CustomTranslationsSectionProps = {
    mobile: false,
    value: "hello\t你好",
    placeholder: "example",
    notice: "",
    summary: "1 条释义",
    saveState: "idle",
    saveError: "",
    onChange: vi.fn(),
    onFlush: vi.fn(),
    ...overrides,
  };
  render(<CustomTranslationsSection {...props} />);
  return props;
}

test("forwards edits and saves at once when the field loses focus, with no save button", () => {
  const props = renderSection();
  const field = screen.getByLabelText("自定义候选释义");

  fireEvent.change(field, { target: { value: "world\t世界" } });
  fireEvent.blur(field);

  expect(props.onChange).toHaveBeenCalledWith("world\t世界");
  expect(props.onFlush).toHaveBeenCalledOnce();
  expect(screen.queryByRole("button", { name: "保存自定义释义" })).toBeNull();
  expect(screen.getByText(/候选窗/)).toBeTruthy();
  // The page's own footer reports the page; the editor keeps its save status to a line of text rather than a second footer.
  expect(document.querySelector("footer")).toBeNull();
  expect(screen.queryByRole("button", { name: "恢复默认设置" })).toBeNull();
});

test("shows the automatic save status", () => {
  renderSection({ saveState: "saving" });
  expect(screen.getByText("正在保存…")).toBeTruthy();
  cleanup();
  renderSection({ saveState: "saved" });
  expect(screen.getByText("已保存")).toBeTruthy();
});

test("a failed save shows why and retries on request", () => {
  const props = renderSection({ mobile: true, saveState: "failed", saveError: "磁盘已满" });

  expect(screen.getByRole("alert").textContent).toBe("磁盘已满");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));

  expect(props.onFlush).toHaveBeenCalledOnce();
  expect(screen.getByText(/候选栏/)).toBeTruthy();
});

test("the notice replaces the summary", () => {
  renderSection({ notice: "自定义释义过大，请精简后再保存。" });
  expect(screen.getByRole("status").textContent).toBe("自定义释义过大，请精简后再保存。");
});
