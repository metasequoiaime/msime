// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TypingStatisticsPage, type TypingStatisticsStatus } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const neverWritten: TypingStatisticsStatus = {
  availability: "neverWritten",
  statistics: { enabled: true, total: 0, days: {} },
};
const ready: TypingStatisticsStatus = {
  availability: "ready",
  statistics: { enabled: true, total: 0, days: {} },
};

function client(status: TypingStatisticsStatus) {
  return { load: async () => status, setEnabled: vi.fn(), reset: vi.fn() };
}

// Telling an iOS user to type more characters is advice that cannot work: without Full Access
// the extension never reaches the shared container, so the count stays at zero however much
// they type. The message has to name the prerequisite.
test("iOS names Full Access and offers the settings entry", async () => {
  const openSystemSettings = vi.fn(async () => {});
  render(
    <TypingStatisticsPage
      client={client(neverWritten)}
      platform="ios"
      openSystemSettings={openSystemSettings}
    />,
  );
  await screen.findByText(/键盘从未写入过统计/);

  expect(screen.getByText(/允许完全访问/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "打开系统键盘设置" }));
  expect(openSystemSettings).toHaveBeenCalled();
});

test("other platforms keep the plain guidance and no settings entry", async () => {
  render(
    <TypingStatisticsPage
      client={client(neverWritten)}
      platform="windows"
      openSystemSettings={vi.fn()}
    />,
  );
  await screen.findByText(/键盘从未写入过统计/);

  expect(screen.queryByText(/允许完全访问/)).toBeNull();
  expect(screen.queryByRole("button", { name: "打开系统键盘设置" })).toBeNull();
});

// A host that cannot open Settings must not render a dead button.
test("iOS without the settings capability still explains the requirement", async () => {
  render(<TypingStatisticsPage client={client(neverWritten)} platform="ios" />);
  await screen.findByText(/允许完全访问/);

  expect(screen.queryByRole("button", { name: "打开系统键盘设置" })).toBeNull();
});

// 统计文件存在不说明键盘写过：在 app 里开启记录就会写出文件，而没有完全访问的键盘什么都不记，计数一直是零。iOS 上计数为零的提示要先提醒完全访问，再说清空过统计之类的其他可能，并给出系统设置入口。
test("iOS zero count with an established file still names Full Access first", async () => {
  const openSystemSettings = vi.fn(async () => {});
  render(
    <TypingStatisticsPage
      client={client({ ...ready, lastWrittenMs: Date.UTC(2026, 9, 9, 8, 30) })}
      platform="ios"
      openSystemSettings={openSystemSettings}
    />,
  );
  const message = (await screen.findByText(/当前计数为零/)).textContent ?? "";

  expect(message.indexOf("允许完全访问")).toBeGreaterThan(-1);
  expect(message.indexOf("允许完全访问")).toBeLessThan(message.indexOf("清空过统计"));
  fireEvent.click(screen.getByRole("button", { name: "打开系统键盘设置" }));
  expect(openSystemSettings).toHaveBeenCalled();
});

test("iOS zero count without a write time names Full Access", async () => {
  render(
    <TypingStatisticsPage client={client(ready)} platform="ios" openSystemSettings={vi.fn()} />,
  );
  await screen.findByText(/统计文件已建立/);

  expect(screen.getByText(/允许完全访问/)).toBeTruthy();
});

// 其他平台没有完全访问这回事，计数为零时不能把人引到它上面。
test("other platforms keep the zero-count message free of Full Access", async () => {
  render(
    <TypingStatisticsPage
      client={client({ ...ready, lastWrittenMs: Date.UTC(2026, 9, 9, 8, 30) })}
      platform="windows"
      openSystemSettings={vi.fn()}
    />,
  );
  await screen.findByText(/当前计数为零/);

  expect(screen.queryByText(/允许完全访问/)).toBeNull();
  expect(screen.queryByRole("button", { name: "打开系统键盘设置" })).toBeNull();
});
