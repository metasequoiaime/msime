// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { StatisticsOptionsMenu } from "../../../../packages/ui/src/settings/statistics-options-menu";
import { retentionChoices } from "../../../../packages/ui/src/settings/typing-statistics-model";

afterEach(cleanup);

const actions = () => ({
  onEnabledChange: vi.fn(),
  onRefresh: vi.fn(),
  onReset: vi.fn(),
});

test("记录开关由外部状态控制，菜单动作只通知调用方", () => {
  const callbacks = actions();
  const { rerender } = render(
    <StatisticsOptionsMenu {...callbacks} enabled={false} busy={false} className="-mb-1" />,
  );
  fireEvent.click(screen.getByLabelText("统计选项", { selector: "summary" }));
  const toggle = screen.getByRole("checkbox", { name: "记录打字统计" }) as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  expect(callbacks.onEnabledChange).toHaveBeenCalledExactlyOnceWith(true);
  expect(toggle.checked).toBe(false);
  fireEvent.click(screen.getByRole("menuitem", { name: "刷新统计" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "清空统计" }));
  expect(callbacks.onRefresh).toHaveBeenCalledOnce();
  expect(callbacks.onReset).toHaveBeenCalledOnce();
  expect(screen.queryByRole("combobox")).toBeNull();
  rerender(<StatisticsOptionsMenu {...callbacks} enabled busy={false} className="-mb-1" />);
  expect(toggle.checked).toBe(true);
});

test("可选保留期使用共享选项和永久保留缺省值，能力移除时隐藏控件", () => {
  const callbacks = actions();
  const onRetentionChange = vi.fn();
  const { rerender } = render(
    <StatisticsOptionsMenu
      {...callbacks}
      enabled
      busy={false}
      className="-mb-3.5"
      onRetentionChange={onRetentionChange}
    />,
  );
  fireEvent.click(screen.getByLabelText("统计选项", { selector: "summary" }));
  const control = screen.getByRole("combobox", { name: "自动清理" }) as HTMLSelectElement;
  expect(control.value).toBe("forever");
  expect(Array.from(control.options, (option) => [option.value, option.textContent])).toEqual(
    retentionChoices,
  );
  fireEvent.change(control, { target: { value: "90d" } });
  expect(onRetentionChange).toHaveBeenCalledExactlyOnceWith("90d");
  rerender(
    <StatisticsOptionsMenu
      {...callbacks}
      enabled
      busy={false}
      className="-mb-3.5"
      retention="90d"
    />,
  );
  expect(screen.queryByRole("combobox", { name: "自动清理" })).toBeNull();
});

test("忙碌更新保留展开状态，并禁用所有变更和刷新入口", () => {
  const callbacks = actions();
  const props = {
    ...callbacks,
    enabled: true,
    className: "-mb-3.5",
    onRetentionChange: vi.fn(),
  };
  const { container, rerender } = render(<StatisticsOptionsMenu {...props} busy={false} />);
  fireEvent.click(screen.getByLabelText("统计选项", { selector: "summary" }));
  expect(container.querySelector("details")?.open).toBe(true);
  rerender(<StatisticsOptionsMenu {...props} busy />);
  expect(container.querySelector("details")?.open).toBe(true);
  expect((screen.getByRole("checkbox") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByRole("combobox") as HTMLSelectElement).disabled).toBe(true);
  const refresh = screen.getByRole("menuitem", { name: "处理中…" }) as HTMLButtonElement;
  const reset = screen.getByRole("menuitem", { name: "清空统计" }) as HTMLButtonElement;
  expect(refresh.disabled).toBe(true);
  expect(refresh.getAttribute("aria-busy")).toBe("true");
  expect(reset.disabled).toBe(true);
  fireEvent.click(refresh);
  fireEvent.click(reset);
  expect(callbacks.onRefresh).not.toHaveBeenCalled();
  expect(callbacks.onReset).not.toHaveBeenCalled();
});
