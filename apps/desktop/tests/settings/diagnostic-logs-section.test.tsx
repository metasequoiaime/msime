// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DiagnosticLogsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("diagnostic logs report server and TSF toggle changes", () => {
  const onChange = vi.fn();
  render(
    <DiagnosticLogsSection
      visible
      linux={false}
      macos={false}
      windows
      values={{ server: false, tsf: false }}
      onChange={onChange}
      onError={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByLabelText("Server 端日志"));
  fireEvent.click(screen.getByLabelText("TSF 端日志"));
  expect(onChange).toHaveBeenNthCalledWith(1, { server: true });
  expect(onChange).toHaveBeenNthCalledWith(2, { tsf: true });
});

test("diagnostic logs expose the macOS directory action", () => {
  const openDirectory = vi.fn(async () => {});
  render(
    <DiagnosticLogsSection
      visible
      linux={false}
      macos
      windows={false}
      values={{ server: true, tsf: false }}
      openDirectory={openDirectory}
      onChange={vi.fn()}
      onError={vi.fn()}
    />,
  );

  expect(screen.getByLabelText("输入法日志")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "在 Finder 中显示" }));
  expect(openDirectory).toHaveBeenCalledOnce();
});
