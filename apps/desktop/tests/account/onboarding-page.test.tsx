// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  WelcomeFlowPage,
  type ImeSetupClient,
  type ImeSetupState,
  type OnboardingActions,
} from "@msime/ui";

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

test("ignores a same-tick duplicate onboarding action", async () => {
  let resolvePrepare!: () => void;
  const prepareResources = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        resolvePrepare = resolve;
      }),
  );
  const actions = makeActions({ prepareResources });
  render(<WelcomeFlowPage actions={actions} onComplete={vi.fn().mockResolvedValue(undefined)} />);

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "下一步" }));
    fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  });
  expect(prepareResources).toHaveBeenCalledOnce();
  resolvePrepare();
  await screen.findByRole("heading", { name: "选择输入方式" });
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

test("a HarmonyOS phone draws the design's step header and keeps the instructions without a setup client", async () => {
  const actions = makeActions({ platform: "harmony" });
  render(<WelcomeFlowPage actions={actions} onComplete={vi.fn().mockResolvedValue(undefined)} />);

  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
  expect(screen.getByText("第一步 · 约 30 秒")).toBeTruthy();
  expect(
    screen.getByText("在系统设置里启用水杉，并设为默认输入法，之后在任何应用里都能直接用。"),
  ).toBeTruthy();
  // 计数在顶部，圆点在步骤下方；没有 logo 页头，也没有「上一步」。
  expect(screen.getByText("1 / 4")).toBeTruthy();
  expect(screen.getByRole("img", { name: "第 1 步，共 4 步" })).toBeTruthy();
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(screen.queryByRole("button", { name: "上一步" })).toBeNull();
  expect(screen.getByText("前往 HarmonyOS 的系统输入法设置。")).toBeTruthy();
  expect(screen.getByText(/系统设置页面由 HarmonyOS 管理/)).toBeTruthy();
  expect(screen.queryByText(/MSIME Preview/)).toBeNull();
  expect(screen.queryByText(/Android/)).toBeNull();
  const shell = screen.getByRole("main", { name: "首次设置" });
  expect(shell.getAttribute("data-platform")).toBe("harmony");
  expect(shell.hasAttribute("data-season")).toBe(false);

  fireEvent.click(screen.getByRole("button", { name: "打开系统设置" }));
  await waitFor(() => expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("button", { name: "选择输入法" }));
  await waitFor(() => expect(actions.showInputMethodPicker).toHaveBeenCalledOnce());
});

function fakeSetup(initial: ImeSetupState | null) {
  let listener: ((state: ImeSetupState) => void) | undefined;
  const client: ImeSetupClient = {
    read: () => initial,
    subscribe: (next) => {
      listener = next;
      return () => {
        listener = undefined;
      };
    },
  };
  return { client, emit: (state: ImeSetupState) => act(() => listener?.(state)) };
}

test("the HarmonyOS setup step shows the host's two checks and the step each still needs", async () => {
  const setup = fakeSetup({ enabled: false, current: false });
  const actions = makeActions({ platform: "harmony" });
  render(
    <WelcomeFlowPage
      actions={actions}
      setup={setup.client}
      onComplete={vi.fn().mockResolvedValue(undefined)}
    />,
  );

  expect(screen.getByText("已在系统中启用")).toBeTruthy();
  expect(screen.getByText("设为默认输入法")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "打开系统设置" })).toBeNull();
  // 尚未启用：系统选择器不会列出它，所以「设为默认」还会打开系统列表。
  fireEvent.click(screen.getByRole("button", { name: "设为默认" }));
  await waitFor(() => expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce());
  expect(actions.prepareResources).toHaveBeenCalledOnce();

  // 从系统设置返回后，宿主报告输入法已启用。
  setup.emit({ enabled: true, current: false });
  expect(screen.queryByRole("button", { name: "去开启" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "设为默认" }));
  await waitFor(() => expect(actions.showInputMethodPicker).toHaveBeenCalledOnce());

  setup.emit({ enabled: true, current: true });
  expect(screen.queryByRole("button", { name: "设为默认" })).toBeNull();
  expect(screen.getAllByText("，已完成")).toHaveLength(2);
});

test("a HarmonyOS check the host has not answered offers no step", () => {
  const setup = fakeSetup(null);
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony" })}
      setup={setup.client}
      onComplete={vi.fn().mockResolvedValue(undefined)}
    />,
  );

  expect(screen.getByText("已在系统中启用")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "去开启" })).toBeNull();
  expect(screen.queryByRole("button", { name: "设为默认" })).toBeNull();
});

test("the HarmonyOS phone walks the design's steps and 跳过 keeps the keyboard chosen", async () => {
  const onSkip = vi.fn().mockResolvedValue(undefined);
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony" })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      onSkip={onSkip}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "选一套输入方案" });
  expect(screen.getByText("第二步 · 随时可以改")).toBeTruthy();
  expect(
    screen.getByText("全拼、9 键、双拼和五笔用的是同一套引擎，词库和自造词通用。"),
  ).toBeTruthy();
  expect(screen.getByRole("radio", { name: /全拼 26 键/ }).getAttribute("aria-checked")).toBe(
    "true",
  );
  expect(screen.getByRole("radio", { name: /全拼 9 键/ })).toBeTruthy();
  expect(screen.getByRole("radio", { name: /五笔.*默认 86 版/ })).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: /双拼.*默认小鹤/ }));
  expect(screen.getByRole("radio", { name: /双拼/ }).getAttribute("aria-checked")).toBe("true");

  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "候选下方就是译文" });
  expect(screen.getByText("候选")).toBeTruthy();
  expect(screen.queryByText("candidate")).toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "显示英文释义" }));
  expect(screen.getByText("candidate")).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "登录后多端同步" });
  expect(screen.getByText("最后一步")).toBeTruthy();
  for (const perk of ["词库", "皮肤与主题", "云剪贴板"])
    expect(screen.getByText(perk)).toBeTruthy();
  expect(screen.getByRole("button", { name: "登录" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "稍后再说" })).toBeTruthy();
  // 最后一步的顶部同样保留「跳过」，离开时带上途中已选的内容。
  fireEvent.click(screen.getByRole("button", { name: "跳过" }));

  await waitFor(() =>
    expect(onSkip).toHaveBeenCalledWith("xiaohe", {
      candidateEnglishGloss: true,
      openAccount: false,
    }),
  );
});

test("跳过 before choosing a keyboard asks the host to keep the saved scheme", async () => {
  const onSkip = vi.fn().mockResolvedValue(undefined);
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony" })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      onSkip={onSkip}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "跳过" }));

  await waitFor(() =>
    expect(onSkip).toHaveBeenCalledWith("quanpin", {
      candidateEnglishGloss: undefined,
      openAccount: false,
      keepScheme: true,
    }),
  );
});

test("signed in, the HarmonyOS last step finishes with 开始使用 and offers no 稍后再说", async () => {
  const onComplete = vi.fn().mockResolvedValue(undefined);
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony" })}
      onComplete={onComplete}
      signedIn
    />,
  );

  for (const heading of ["选一套输入方案", "候选下方就是译文", "登录后多端同步"]) {
    fireEvent.click(screen.getByRole("button", { name: "下一步" }));
    await screen.findByRole("heading", { name: heading });
  }
  expect(screen.queryByRole("button", { name: "稍后再说" })).toBeNull();
  expect(screen.queryByRole("button", { name: "登录" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "开始使用" }));

  await waitFor(() =>
    expect(onComplete).toHaveBeenCalledWith("quanpin", {
      candidateEnglishGloss: undefined,
      openAccount: false,
    }),
  );
});

test("HarmonyOS offers only the keyboards the version and the host have", async () => {
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony" })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      edition={{
        id: "pinyin",
        input_schemes: ["quanpin", "shuangpin", "wubi"],
        default_scheme: "quanpin",
        temporary_japanese: true,
        neural_keyboard: true,
        offline_glosses: true,
        handwriting: true,
        wubi_mixed_pinyin_default: false,
      }}
      inputSchemes={["quanpin", "wubi"]}
      wubiProfile="wubi98"
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "选一套输入方案" });
  expect(screen.queryByRole("radio", { name: /双拼/ })).toBeNull();
  expect(screen.getByRole("radio", { name: /五笔.*当前 98 版/ })).toBeTruthy();
  expect(screen.getByText("全拼、9 键和五笔用的是同一套引擎，词库和自造词通用。")).toBeTruthy();
});

test("the HarmonyOS phone turns pages with a horizontal swipe and steps back through the history", async () => {
  const onComplete = vi.fn().mockResolvedValue(undefined);
  render(
    <WelcomeFlowPage actions={makeActions({ platform: "harmony" })} onComplete={onComplete} />,
  );
  const shell = screen.getByRole("main", { name: "首次设置" });
  const swipe = (dx: number, dy = 0) => {
    fireEvent.pointerDown(shell, { clientX: 200, clientY: 300 });
    fireEvent.pointerUp(shell, { clientX: 200 + dx, clientY: 300 + dy });
  };

  // 滑动太短，或纵向位移大于横向，都不算翻页。
  swipe(-40);
  swipe(-60, 50);
  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
  // 第一步之前没有别的步骤。
  swipe(80);
  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();

  swipe(-80);
  await screen.findByRole("heading", { name: "选一套输入方案" });
  swipe(-80);
  await screen.findByRole("heading", { name: "候选下方就是译文" });
  // 系统返回手势让 WebView 后退一条记录，也就是一步。
  act(() => window.history.back());
  await screen.findByRole("heading", { name: "选一套输入方案" });
  swipe(80);
  await screen.findByRole("heading", { name: "把水杉加进键盘" });

  for (const heading of ["选一套输入方案", "候选下方就是译文", "登录后多端同步"]) {
    swipe(-80);
    await screen.findByRole("heading", { name: heading });
  }
  // 最后一步不能再往后滑。
  swipe(-80);
  expect(screen.getByRole("heading", { name: "登录后多端同步" })).toBeTruthy();
  const depth = window.history.length;
  fireEvent.click(screen.getByRole("button", { name: "稍后再说" }));
  // 流程在交接前把自己的记录从栈中移除，让之后的页面拿到干净的历史。
  await waitFor(() => expect(onComplete).toHaveBeenCalledOnce());
  expect(window.history.state?.msimeOnboarding).toBeUndefined();
  expect(window.history.length).toBe(depth);
});

test("a HarmonyOS phone takes the app theme's season on its root", () => {
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony" })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      appTheme={{
        id: "qiushan",
        season: "autumn",
        accent: "#B5562B",
        accent_soft: "#B5562B22",
        on_accent: "#FFFFFF",
        background: "#F6E9DC",
        card: "#FFFBF6",
        hair: "#E8D6C4",
      }}
    />,
  );

  const shell = screen.getByRole("main", { name: "首次设置" });
  expect(shell.getAttribute("data-season")).toBe("autumn");
  expect(shell.style.getPropertyValue("--accent-color")).toBe("#B5562B");
  expect(shell.style.getPropertyValue("--p-group-bg")).toBe("#FFFBF6");
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
  // 桌面端措辞：输入法在系统自己的设置里设定。
  expect(screen.getByRole("heading", { name: "设为系统输入法" })).toBeTruthy();
  expect(screen.getByText("在 设置 → 系统 → 输入法 里启用水杉，并设为默认输入法。")).toBeTruthy();
  const shell = screen.getByRole("main", { name: "首次设置" });
  expect(shell.getAttribute("data-platform")).toBe("hm2");
  expect(shell.hasAttribute("data-mobile")).toBe(false);
  expect(screen.getByText("第 1 步，共 4 步")).toBeTruthy();
  expect(screen.queryByRole("progressbar")).toBeNull();
});

test("the HarmonyOS 2-in-1 has no 9 键 and keeps 跳过 in its bar on every step", async () => {
  const onSkip = vi.fn().mockResolvedValue(undefined);
  const setup = fakeSetup({ enabled: false, current: false });
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony", mobileSettings: false })}
      setup={setup.client}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      onSkip={onSkip}
    />,
  );

  expect(screen.getByText("已在「系统 → 输入法」中启用")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "选一套输入方案" });
  expect(screen.queryByRole("radio", { name: /9 键/ })).toBeNull();
  expect(screen.getByText("全拼、双拼和五笔用的是同一套引擎，词库和自造词通用。")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "上一步" }));
  await screen.findByRole("heading", { name: "设为系统输入法" });
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "选一套输入方案" });
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "候选下方就是译文" });
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "登录后多端同步" });
  fireEvent.click(screen.getByRole("button", { name: "跳过" }));

  // 直接走过方案这一步，即接受它预选的键盘。
  await waitFor(() =>
    expect(onSkip).toHaveBeenCalledWith("quanpin", {
      candidateEnglishGloss: undefined,
      openAccount: false,
    }),
  );
});

test("a HarmonyOS phone keeps the splash, which says nothing about tapping", () => {
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony", mobileSettings: true })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      splash
    />,
  );

  expect(screen.getByRole("button", { name: "跳过开屏" })).toBeTruthy();
  expect(screen.queryByText("轻点跳过")).toBeNull();
  expect(screen.getByText("METASEQUOIA IME")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "跳过开屏" }));
  expect(screen.getByRole("heading", { name: "把水杉加进键盘" })).toBeTruthy();
});

// 设置页挂载后由它为 HarmonyOS 系统栏着色；在那之前由引导流程着色，这样启动画面覆盖状态栏，各步骤上方的系统栏与该步骤页面同色。
test("a HarmonyOS phone colours the system bars to the splash and then to the steps' page", () => {
  const setSystemBars = vi.fn();
  const autumn = {
    id: "siji",
    season: "autumn",
    accent: "#B5562B",
    accent_soft: "#F3E1D6",
    on_accent: "#FFFFFF",
    background: "#F6E9DC",
    card: "#FFFBF6",
    hair: "#EADBCB",
  } as const;
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony", mobileSettings: true })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      appTheme={autumn}
      chrome={{ setSystemBars }}
      splash
    />,
  );

  // 启动画面底色是季节强调色以 20% 叠在 `#0A0B0A` 上，内容为浅色。
  expect(setSystemBars).toHaveBeenLastCalledWith({
    background: "#2c1a11",
    navigationBar: "#2c1a11",
    dark: true,
  });
  fireEvent.click(screen.getByRole("button", { name: "跳过开屏" }));
  expect(setSystemBars).toHaveBeenLastCalledWith({
    background: autumn.background,
    navigationBar: autumn.background,
    dark: false,
  });
  cleanup();

  // 2in1 没有需要着色的手机系统栏。
  const desktopBars = vi.fn();
  render(
    <WelcomeFlowPage
      actions={makeActions({ platform: "harmony", mobileSettings: false })}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      appTheme={autumn}
      chrome={{ setSystemBars: desktopBars }}
    />,
  );
  expect(desktopBars).not.toHaveBeenCalled();
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

test("the wubi edition skips choosing a keyboard and completes with Wubi", async () => {
  const onComplete = vi.fn().mockResolvedValue(undefined);
  render(
    <WelcomeFlowPage
      actions={makeActions()}
      onComplete={onComplete}
      edition={{
        id: "wubi",
        input_schemes: ["wubi"],
        default_scheme: "wubi",
        temporary_japanese: false,
        neural_keyboard: false,
        offline_glosses: true,
        handwriting: true,
        wubi_mixed_pinyin_default: true,
      }}
    />,
  );

  const progress = () => screen.getByRole("progressbar", { name: "设置进度" });
  expect(progress().getAttribute("aria-valuemax")).toBe("3");
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  // 「选择输入方式」只有全拼的键盘可选，五笔版直接到下一步。
  await screen.findByRole("heading", { name: "候选下方显示译文" });
  expect(screen.queryByRole("radio", { name: /全拼/ })).toBeNull();
  expect(progress().getAttribute("aria-valuenow")).toBe("2");
  fireEvent.click(screen.getByRole("button", { name: "上一步" }));
  await screen.findByRole("heading", { name: "把水杉加进键盘" });
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "候选下方显示译文" });
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "登录后多端同步" });
  expect(progress().getAttribute("aria-valuenow")).toBe("3");
  fireEvent.click(screen.getByRole("button", { name: "稍后再说" }));

  await waitFor(() =>
    expect(onComplete).toHaveBeenCalledWith("wubi", {
      candidateEnglishGloss: undefined,
      openAccount: false,
    }),
  );
});

test("a wubi edition learned only after preparing resources still skips choosing a keyboard", async () => {
  // 第一次启动：发现宿主能力时还没有 HostOptions，版本要等第一步准备好资源之后才知道。
  const onComplete = vi.fn().mockResolvedValue(undefined);
  const wubi = {
    id: "wubi",
    input_schemes: ["wubi" as const],
    default_scheme: "wubi" as const,
    temporary_japanese: false,
    neural_keyboard: false,
    offline_glosses: true,
    handwriting: true,
    wubi_mixed_pinyin_default: true,
  };
  let learnEdition = () => {};
  const actions = makeActions({
    prepareResources: vi.fn(async () => learnEdition()),
  });
  const { rerender } = render(<WelcomeFlowPage actions={actions} onComplete={onComplete} />);
  learnEdition = () =>
    rerender(<WelcomeFlowPage actions={actions} onComplete={onComplete} edition={wubi} />);

  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "候选下方显示译文" });
  expect(screen.queryByRole("radio", { name: /全拼/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  await screen.findByRole("heading", { name: "登录后多端同步" });
  fireEvent.click(screen.getByRole("button", { name: "稍后再说" }));

  await waitFor(() =>
    expect(onComplete).toHaveBeenCalledWith("wubi", {
      candidateEnglishGloss: undefined,
      openAccount: false,
    }),
  );
});

test("the pinyin edition keeps the keyboard choice", async () => {
  render(
    <WelcomeFlowPage
      actions={makeActions()}
      onComplete={vi.fn().mockResolvedValue(undefined)}
      edition={{
        id: "pinyin",
        input_schemes: ["quanpin", "shuangpin"],
        default_scheme: "quanpin",
        temporary_japanese: true,
        neural_keyboard: true,
        offline_glosses: true,
        handwriting: true,
        wubi_mixed_pinyin_default: false,
      }}
    />,
  );

  expect(screen.getByRole("progressbar", { name: "设置进度" }).getAttribute("aria-valuemax")).toBe(
    "4",
  );
  fireEvent.click(screen.getByRole("button", { name: "下一步" }));
  expect(await screen.findByRole("heading", { name: "选择输入方式" })).toBeTruthy();
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
