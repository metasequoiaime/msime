// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MacosInputModeEntriesSection, type MacosInputModesClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const prefix = "app.msime.inputmethod.MetasequoiaIME.";
const allSchemes = [
  "quanpin",
  "shuangpin",
  "wubi",
  "japanese",
  "korean",
  "cantonese",
  "zhuyin",
  "vietnamese",
] as const;

function client(enabled: string[] | null, openSettings = vi.fn(() => Promise.resolve())) {
  return {
    enabled: vi.fn(() => Promise.resolve(enabled)),
    openSettings,
  } satisfies MacosInputModesClient;
}

test("names the language each missing entry sits under in the add dialog", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client(
        ["Hans", "Shuangpin", "Wubi", "Roman", "Japanese", "Korean", "Vietnamese"].map(
          (mode) => prefix + mode,
        ),
      )}
      scheme="quanpin"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  const text = (await screen.findByText(/还没加入的/)).textContent;
  expect(text).toContain("「水杉输入法 · 粤」在「粤语」下");
  expect(text).toContain("「水杉输入法 · 注」在「繁体中文」下");
  expect(text).not.toContain("水杉输入法 · 双");
  expect(text).not.toContain("菜单栏里还没有");
  expect(screen.getByRole("button", { name: "打开键盘设置" })).toBeTruthy();
});

test("leads with the current scheme's entry when it is missing", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client([prefix + "Hans"])}
      scheme="cantonese"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  const text = (await screen.findByText(/还没加入的/)).textContent ?? "";
  expect(text.startsWith("菜单栏里还没有「水杉输入法 · 粤」")).toBe(true);
});

test("leaves out the entries of schemes the host does not offer", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client([prefix + "Hans", prefix + "Roman"])}
      scheme="quanpin"
      inputSchemes={["quanpin"]}
      onError={vi.fn()}
    />,
  );

  expect(await screen.findByText("输入法菜单里已经有水杉输入法的全部入口。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "打开键盘设置" })).toBeNull();
});

test("shows nothing when the input source list cannot be read", async () => {
  const modes = client(null);
  const { container } = render(
    <MacosInputModeEntriesSection
      client={modes}
      scheme="cantonese"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  await vi.waitFor(() => expect(modes.enabled).toHaveBeenCalled());
  expect(container.textContent).toBe("");
});

test("reads the list again while the user is in System Settings", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const enabled = vi
    .fn<() => Promise<string[] | null>>()
    .mockResolvedValueOnce([prefix + "Hans"])
    .mockResolvedValue([prefix + "Hans", prefix + "Cantonese"]);
  render(
    <MacosInputModeEntriesSection
      client={{ enabled, openSettings: () => Promise.resolve() }}
      scheme="cantonese"
      inputSchemes={["quanpin", "cantonese"]}
      onError={vi.fn()}
    />,
  );

  fireEvent.click(await screen.findByRole("button", { name: "打开键盘设置" }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(3000);
  });
  expect(await screen.findByText(/还没加入的：「水杉输入法 · 英」/)).toBeTruthy();
  expect(screen.queryByText(/菜单栏里还没有/)).toBeNull();
});

test("reports a failure when opening keyboard settings fails", async () => {
  const onError = vi.fn();
  render(
    <MacosInputModeEntriesSection
      client={client(
        [prefix + "Hans"],
        vi.fn(() => Promise.reject(new Error("unavailable"))),
      )}
      scheme="quanpin"
      inputSchemes={allSchemes}
      onError={onError}
    />,
  );

  fireEvent.click(await screen.findByRole("button", { name: "打开键盘设置" }));
  await vi.waitFor(() =>
    expect(onError).toHaveBeenCalledWith(
      "无法打开系统设置，请手动前往「系统设置 › 键盘 › 文字输入 › 输入法」。",
    ),
  );
});
