// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  TelemetrySection,
  UsageReportingDetails,
  usageReportingEndpoint,
  usageReportingSummary,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("usage reporting switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<TelemetrySection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("匿名使用统计"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("usage reporting is on when the preference is absent", () => {
  render(<TelemetrySection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("匿名使用统计") as HTMLInputElement).checked).toBe(true);
});

test("the switch carries one line and a row that opens what is sent", () => {
  const onOpenDetails = vi.fn();
  render(<TelemetrySection value={true} onChange={vi.fn()} onOpenDetails={onOpenDetails} />);

  expect(screen.getByText(usageReportingSummary)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "发送哪些内容" }));
  expect(onOpenDetails).toHaveBeenCalledTimes(1);
});

test("without a way to open the page there is no row pointing at it", () => {
  render(<TelemetrySection value={true} onChange={vi.fn()} />);

  expect(screen.queryByRole("button", { name: "发送哪些内容" })).toBeNull();
});

test("what is sent is one fact per row, with the endpoint on its own", () => {
  render(<UsageReportingDetails />);

  expect(usageReportingEndpoint).toBe("https://api.msime.app/v1/telemetry/events");
  expect(screen.getByText(usageReportingEndpoint)).toBeTruthy();
  for (const fact of [
    "每天最多一条活跃记录",
    "本机随机生成的安装 id",
    "输入内容、候选和剪贴板",
    "不再发送，并清空尚未发送的记录",
  ]) {
    expect(screen.getByText(fact)).toBeTruthy();
  }
});

test("usage reporting shows off once the user turned it off", () => {
  render(<TelemetrySection value={false} onChange={vi.fn()} />);

  expect((screen.getByLabelText("匿名使用统计") as HTMLInputElement).checked).toBe(false);
});
