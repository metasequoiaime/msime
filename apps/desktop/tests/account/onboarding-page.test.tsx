// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { WelcomeFlowPage, type OnboardingActions } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function makeActions(overrides: Partial<OnboardingActions> = {}): OnboardingActions {
  return {
    prepareResources: vi.fn().mockResolvedValue(undefined),
    openSystemKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    showInputMethodPicker: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

test("starts on the add-the-keyboard step", () => {
  render(
    <WelcomeFlowPage actions={makeActions()} onComplete={vi.fn().mockResolvedValue(undefined)} />,
  );

  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "下一步" })).toBeTruthy();
  // Android draws its progress as a linear bar with a pair of buttons under the page, and the bar replaces the count.
  expect(screen.getByRole("progressbar", { name: "设置进度" }).getAttribute("aria-valuenow")).toBe(
    "1",
  );
  expect(screen.queryByText("1 / 4")).toBeNull();
  expect(screen.queryByRole("button", { name: "上一步" })).toBeNull();
});

test("prepares resources before leaving the Android setup step", async () => {
  const actions = makeActions();
  render(<WelcomeFlowPage actions={actions} onComplete={vi.fn().mockResolvedValue(undefined)} />);

  fireEvent.click(screen.getByRole("button", { name: "下一步" }));

  await waitFor(() => expect(actions.prepareResources).toHaveBeenCalledOnce());
  expect(await screen.findByRole("heading", { name: "选择输入方式" })).toBeTruthy();
  expect(screen.getByRole("progressbar", { name: "设置进度" }).getAttribute("aria-valuenow")).toBe(
    "2",
  );
});

test("ignores a second onboarding action while resources are pending", async () => {
  let resolveResources!: () => void;
  const prepareResources = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        resolveResources = resolve;
      }),
  );
  render(
    <WelcomeFlowPage
      actions={makeActions({ prepareResources })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
    />,
  );

  const next = screen.getByRole("button", { name: "下一步" });
  act(() => {
    fireEvent.click(next);
    fireEvent.click(next);
  });
  expect(prepareResources).toHaveBeenCalledOnce();
  resolveResources();
  await waitFor(() => expect(screen.getByRole("heading", { name: "选择输入方式" })).toBeTruthy());
});

test("Android's first step leaves through 跳过 in the button bar, after the dictionaries are in place", async () => {
  const onSkip = vi.fn().mockResolvedValue(undefined);
  const prepareResources = vi.fn().mockResolvedValue(undefined);
  const actions = makeActions({ prepareResources });
  render(
    <WelcomeFlowPage
      actions={actions}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      onSkip={onSkip}
    />,
  );

  // One 跳过, in the bar where 上一步 goes on later steps; Android has none in the header.
  fireEvent.click(screen.getByRole("button", { name: "跳过" }));

  await waitFor(() => expect(onSkip).toHaveBeenCalledOnce());
  expect(prepareResources).toHaveBeenCalledOnce();
  expect(prepareResources.mock.invocationCallOrder[0]).toBeLessThan(
    onSkip.mock.invocationCallOrder[0],
  );
});

test("prepares resources once, before the first system action", async () => {
  const actions = makeActions();
  render(<WelcomeFlowPage actions={actions} onComplete={vi.fn().mockResolvedValue(undefined)} />);

  fireEvent.click(screen.getByRole("button", { name: "打开系统设置" }));
  await waitFor(() => expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce());
  expect(actions.prepareResources).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "选择输入法" }));
  await waitFor(() => expect(actions.showInputMethodPicker).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "选择输入方式" });
  expect(actions.prepareResources).toHaveBeenCalledOnce();
});

test("adapts setup copy and actions for HarmonyOS", async () => {
  const actions = makeActions({ platform: "harmony" });
  render(<WelcomeFlowPage actions={actions} onComplete={vi.fn().mockResolvedValue(undefined)} />);

  expect(screen.getByText(/HarmonyOS 中启用并选择水杉输入法/)).toBeTruthy();
  expect(screen.getByText("前往 HarmonyOS 的系统输入法设置。")).toBeTruthy();
  expect(screen.getByText(/系统设置页面由 HarmonyOS 管理/)).toBeTruthy();
  expect(screen.queryByText(/Android/)).toBeNull();
  // Only Android draws the linear bar; the other phones keep the page dots.
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(screen.getByRole("img", { name: "第 1 步，共 4 步" })).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "打开系统设置" }));
  await waitFor(() => expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("button", { name: "选择输入法" }));
  await waitFor(() => expect(actions.showInputMethodPicker).toHaveBeenCalledOnce());
});

test("adapts the setup step for iOS keyboard settings", () => {
  const actions = makeActions({ platform: "ios" });
  render(<WelcomeFlowPage actions={actions} onComplete={vi.fn().mockResolvedValue(undefined)} />);
  expect(screen.getByRole("heading", { name: "添加水杉键盘" })).toBeTruthy();
  expect(screen.getByText(/通用 → 键盘 → 键盘/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "选择输入法" })).toBeNull();
});

test("lets iOS users postpone onboarding without showing Android-only actions", async () => {
  const onSkip = vi.fn().mockResolvedValue(undefined);
  const actions = makeActions({ platform: "ios" });
  render(
    <WelcomeFlowPage
      actions={actions}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      onSkip={onSkip}
    />,
  );

  expect(screen.getByText("1 / 4")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "跳过" }));

  await waitFor(() => expect(onSkip).toHaveBeenCalledOnce());
  expect(screen.queryByRole("button", { name: "选择输入法" })).toBeNull();
});

test("a HarmonyOS 2-in-1 takes the desktop sheet and never opens on the splash", () => {
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony", mobileSettings: false })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      onSkip={vi.fn().mockResolvedValue(undefined)}
      splash
    />,
  );

  expect(screen.queryByRole("button", { name: "跳过开屏" })).toBeNull();
  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
  const shell = screen.getByRole("main", { name: "首次设置" });
  expect(shell.getAttribute("data-platform")).toBe("hm2");
  expect(shell.hasAttribute("data-mobile")).toBe(false);
  expect(screen.getByText("第 1 步，共 4 步")).toBeTruthy();
  expect(screen.queryByRole("progressbar")).toBeNull();
});

test("a HarmonyOS phone keeps the splash", () => {
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony", mobileSettings: true })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      splash
    />,
  );

  expect(screen.getByRole("button", { name: "跳过开屏" })).toBeTruthy();
});

async function walkToLastStep() {
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "选择输入方式" });
  fireEvent.click(screen.getByRole("radio", { name: /全拼 9 键/ }));
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "候选下方显示译文" });
}

test("passes the nine-key choice and leaves the gloss alone when its switch is untouched", async () => {
  const onComplete = vi.fn().mockResolvedValue(undefined);
  render(<WelcomeFlowPage actions={makeActions()} onComplete={onComplete} />);

  await walkToLastStep();
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "登录后多端同步" });
  fireEvent.click(screen.getByRole("button", { name: "稍后再说" }));

  await waitFor(() =>
    expect(onComplete).toHaveBeenCalledWith("nine_key", {
      candidateEnglishGloss: undefined,
      openAccount: false,
    }),
  );
});

test("the translation step turns the gloss on and 登录 asks for 我的", async () => {
  const onComplete = vi.fn().mockResolvedValue(undefined);
  render(<WelcomeFlowPage actions={makeActions()} onComplete={onComplete} />);

  await walkToLastStep();
  expect(screen.queryByText("hello")).toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "显示英文释义" }));
  expect(screen.getAllByText("hello").length).toBeGreaterThan(0);
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "登录后多端同步" });
  fireEvent.click(screen.getByRole("button", { name: "登录" }));

  await waitFor(() =>
    expect(onComplete).toHaveBeenCalledWith("nine_key", {
      candidateEnglishGloss: true,
      openAccount: true,
    }),
  );
});

test("reports a resource preparation failure", async () => {
  const actions = makeActions({
    prepareResources: vi.fn().mockRejectedValue(new Error("bootstrap")),
  });
  render(<WelcomeFlowPage actions={actions} onComplete={vi.fn().mockResolvedValue(undefined)} />);

  fireEvent.click(screen.getByRole("button", { name: "下一步" }));

  expect((await screen.findByRole("alert")).textContent).toContain("操作失败");
  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
  // The system actions must not open while the dictionaries are missing.
  fireEvent.click(screen.getByRole("button", { name: "打开系统设置" }));
  await waitFor(() => expect(actions.prepareResources).toHaveBeenCalledTimes(2));
  expect(actions.openSystemKeyboardSettings).not.toHaveBeenCalled();
});

test("a first launch opens on the splash, which a tap skips", () => {
  render(
    <WelcomeFlowPage
      actions={makeActions()}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      splash
    />,
  );

  expect(screen.queryByRole("heading", { name: "把水杉加进键盘" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "跳过开屏" }));
  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
});

test("the splash gives way to the flow on its own", () => {
  vi.useFakeTimers();
  render(
    <WelcomeFlowPage
      actions={makeActions()}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      splash
    />,
  );

  act(() => vi.advanceTimersByTime(2799));
  expect(screen.getByRole("button", { name: "跳过开屏" })).toBeTruthy();
  act(() => vi.advanceTimersByTime(1));
  expect(screen.queryByRole("button", { name: "跳过开屏" })).toBeNull();
  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
});
