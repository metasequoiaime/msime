// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type MobileKeyboardFeedback, type Snapshot } from "@msime/ui";
import { doubaoServiceStatus } from "../../../../packages/ui/src/settings/voice-settings-content";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

function snapshot(preferences: Record<string, unknown> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 3,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      ...preferences,
    },
  } as Snapshot;
}

const padFeedback: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: false,
  hapticStrength: "medium",
  glideTyping: false,
  handwritingDelayMs: 600,
  handwritingStrokeColor: "follow_skin",
  handwritingStrokeWidth: 3,
};

function renderSettings({
  page,
  preferences,
  mobile = true,
  feedback,
  devices = false,
  voiceHotkeys,
  aiGeneration,
}: {
  page: string;
  preferences?: Record<string, unknown>;
  mobile?: boolean;
  feedback?: MobileKeyboardFeedback;
  /** 宿主处理实体键盘的语音快捷键；真机的鸿蒙宿主在手机和 2in1 上都报告 true。 */
  voiceHotkeys?: boolean;
  /** 宿主能列出录音设备，语音页显示「录音设备」分组。 */
  devices?: boolean;
  /** 宿主报的 `ai_generation`；缺省按鸿蒙现在的值（关），语音页没有 AI 润色。 */
  aiGeneration?: boolean;
}) {
  const save = vi.fn().mockImplementation(async (_revision: number, next: unknown) => ({
    ...snapshot(),
    revision: 4,
    preferences: next,
  }));
  const saveFeedback = vi.fn(async (next: MobileKeyboardFeedback) => next);
  render(
    <SettingsPage
      initialPage={page}
      client={{
        load: vi.fn().mockResolvedValue(snapshot(preferences)),
        save,
        host: testHost({
          platform: "harmony",
          mobile_settings: mobile,
          panel_windows: !mobile,
          // 鸿蒙的设置桥只在手机上报告工具栏的显示方式和工具开关（`SettingsFormFactorCapabilities`）。
          touch_toolbar_tools: mobile,
          voice_capture_devices: devices,
          ...(voiceHotkeys === undefined ? {} : { voice_hotkeys: voiceHotkeys }),
          ...(aiGeneration === undefined ? {} : { ai_generation: aiGeneration }),
        }),
        ...(devices ? { listVoiceCaptureDevices: vi.fn().mockResolvedValue([]) } : {}),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        ...(feedback
          ? {
              mobileKeyboardFeedback: {
                load: vi.fn().mockResolvedValue(feedback),
                save: saveFeedback,
              },
            }
          : {}),
      }}
    />,
  );
  return { save, saveFeedback };
}

const groupTitles = (root: HTMLElement) =>
  [...root.querySelectorAll("[data-group-title]")].map((node) => node.textContent ?? "");

/** 打开 HarmonyOS 选择器行的面板并返回它。 */
function openSheet(title: string) {
  const row = screen
    .getAllByRole("button")
    .find(
      (button) =>
        button.getAttribute("aria-haspopup") === "dialog" &&
        button.querySelector("[data-row-title]")?.textContent === title,
    );
  if (!row) throw new Error(`no picker row ${title}`);
  fireEvent.click(row);
  return screen.getByRole("dialog", { name: title });
}

function shownValue(title: string) {
  const row = screen
    .getAllByRole("button")
    .find((button) => button.querySelector("[data-row-title]")?.textContent === title);
  if (!row) throw new Error(`no picker row ${title}`);
  return row;
}

/** 手机语音页上点「服务配置」，打开收着密钥、接口、润色和录音设备的子视图。 */
function openServiceConfig() {
  fireEvent.click(screen.getByRole("button", { name: "服务配置" }));
  return screen.getByRole("dialog", { name: "服务配置" });
}

// ---- 语音输入 ----

// Ctrl+F9、长按右 Alt 这些语音快捷键要连实体键盘才用得上：手机上不列，2in1 照旧列。
test("voice shortcuts for a physical keyboard are listed on a 2in1 and not on a phone", async () => {
  const preferences = { voice_input: { enabled: true, language: "", asr_provider: "doubao" } };
  renderSettings({ page: "voice", preferences, voiceHotkeys: true });
  await settingsFormReady();
  expect(screen.queryByText("语音快捷键")).toBeNull();
  expect(screen.queryByRole("switch", { name: "Ctrl+F9 切换语音" })).toBeNull();
  cleanup();

  renderSettings({ page: "voice", preferences, mobile: false, voiceHotkeys: true });
  await settingsFormReady();
  expect(screen.getByText("语音快捷键")).toBeTruthy();
  // 2in1 的实体键盘把空格锁定固定为开（KeyboardSession 不再读存下的值），选了长按的组合键也不给开关。
  expect((screen.getByRole("combobox", { name: "长按录音" }) as HTMLSelectElement).value).toBe(
    "hotkey_ralt",
  );
  expect(screen.queryByRole("switch", { name: "长按录音时按空格锁定" })).toBeNull();
  expect(screen.getByText(/按住期间按空格锁定录音/)).toBeTruthy();
});

test("the phone voice page puts language and punctuation first and the service below", async () => {
  renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "语音输入" });
  const titles = groupTitles(page);
  expect(titles.slice(0, 2)).toEqual(["识别", "识别服务"]);
  // 介绍说明现在是「识别服务」的页脚，不再单独成组。
  expect(titles).not.toContain("HarmonyOS 输入法语音");
  // 页脚只说用户需要知道的一句：用的是云端识别，去「服务配置」填 API Key。
  expect(within(page).getByText(/使用云端语音识别，先在「服务配置」里填好 API Key/)).toBeTruthy();
  expect(within(page).queryByText(/16 kHz/)).toBeNull();
  const recognition = within(page).getByRole("region", { name: "识别" });
  expect(within(recognition).getByText("自动添加标点")).toBeTruthy();
  const service = within(page).getByRole("region", { name: "识别服务" });
  expect(within(service).getByRole("switch", { name: "启用语音输入" })).toBeTruthy();
  expect(within(service).getByRole("combobox", { name: "识别服务" })).toBeTruthy();
  // HarmonyOS 上没有任何东西实现这些，所以不提供。
  for (const missing of ["离线识别", "启动方式", "上传语音以改进识别", "隐私"]) {
    expect(within(page).queryByText(missing)).toBeNull();
  }
  // 空语言即普通话，所有识别器都默认使用它。
  expect(shownValue("识别语言").textContent).toContain("普通话");
});

test("the recognition language is one of four named choices", async () => {
  const { save } = renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "zh-cn", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  const sheet = openSheet("识别语言");
  expect(
    within(sheet)
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(expect.arrayContaining(["普通话", "粤语", "英语", "普通话 + 英语"]));
  fireEvent.click(within(sheet).getByRole("button", { name: "粤语" }));
  expect(shownValue("识别语言").textContent).toContain("粤语");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].voice_input.language).toBe("yue");
});

test("a language outside the four is shown as stored", async () => {
  renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "ja", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  expect(shownValue("识别语言").textContent).toContain("ja");
});

test("the system recognizer only offers Mandarin and is given it as a locale", async () => {
  const { save } = renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "zh-cn", asr_provider: "system" } },
  });
  await settingsFormReady();
  expect(screen.getByText("系统识别只支持普通话")).toBeTruthy();
  const sheet = openSheet("识别语言");
  for (const label of ["粤语", "英语", "普通话 + 英语"]) {
    expect((within(sheet).getByRole("button", { name: label }) as HTMLButtonElement).disabled).toBe(
      true,
    );
  }
  expect(
    (within(sheet).getByRole("button", { name: "普通话" }) as HTMLButtonElement).disabled,
  ).toBe(false);
  fireEvent.keyDown(sheet, { key: "Escape" });

  // 从云端识别服务切换到系统识别器时，把普通话固定为其 locale 形式。
  cleanup();
  const cloud = renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "en", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  fireEvent.change(screen.getByRole("combobox", { name: "识别服务" }), {
    target: { value: "system" },
  });
  saveSettingsNow();
  await waitFor(() => expect(cloud.save).toHaveBeenCalled());
  const voice = cloud.save.mock.calls.at(-1)?.[1].voice_input;
  expect(voice.asr_provider).toBe("system");
  expect(voice.language).toBe("zh-CN");
  expect(save).not.toHaveBeenCalled();
});

// 主页面只留普通人会调的：识别三项、服务和它的配置入口、两个录音开关。密钥、接口、润色服务、提示音细分和录音设备都在「服务配置」子视图里，返回回到语音页。
test("the phone voice page keeps service details behind 服务配置", async () => {
  renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "", asr_provider: "doubao" } },
    devices: true,
  });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "语音输入" });
  expect(groupTitles(page)).toEqual(["识别", "识别服务", "录音"]);
  for (const hidden of ["Doubao API Key", "流式接口", "录音设备", "开始录音提示音", "文本润色"]) {
    expect(within(page).queryByText(hidden)).toBeNull();
  }
  expect(within(page).getByRole("switch", { name: "提示音" })).toBeTruthy();
  const entry = screen.getByRole("button", { name: "服务配置" });
  // 出厂的豆包没有任何凭据，键盘实际用的是系统识别，要照实写出来。
  expect(entry.textContent).toContain("未填写，暂用系统识别");

  const dialog = openServiceConfig();
  expect(within(dialog).getByLabelText("Doubao API Key")).toBeTruthy();
  // HarmonyOS 只认提示音总开关，子视图不再画开始、结束提示音。
  expect(within(dialog).queryByRole("switch", { name: "开始录音提示音" })).toBeNull();
  expect(within(dialog).queryByRole("switch", { name: "结束录音提示音" })).toBeNull();
  expect(within(dialog).queryByRole("region", { name: "提示音" })).toBeNull();
  expect(within(dialog).getByRole("region", { name: "录音设备" })).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button", { name: "返回" }));
  expect(await screen.findByRole("button", { name: "服务配置" })).toBeTruthy();
  expect(screen.queryByRole("dialog", { name: "服务配置" })).toBeNull();
});

// 豆包的状态与键盘的判断一致：密钥可以在分槽里；一项都没填是系统识别，填了一部分是没填完整。
test("the Doubao service row tells unconfigured, partial and complete apart", () => {
  const base = { enabled: true, language: "", asr_provider: "doubao" } as const;
  expect(doubaoServiceStatus({ ...base })).toBe("未填写，暂用系统识别");
  expect(doubaoServiceStatus({ ...base, asr_token: "fixture-key" })).toBe("已填写");
  expect(doubaoServiceStatus({ ...base, asr_tokens: { doubao: "fixture-key" } })).toBe("已填写");
  expect(doubaoServiceStatus({ ...base, asr_app_key: "fixture-app" })).toBe("未填写完整");
  expect(
    doubaoServiceStatus({ ...base, doubao_auth_mode: "legacy", asr_token: "fixture-key" }),
  ).toBe("未填写完整");
  expect(
    doubaoServiceStatus({
      ...base,
      doubao_auth_mode: "legacy",
      asr_token: "fixture-key",
      asr_app_key: "fixture-app",
    }),
  ).toBe("已填写");
});

// 系统识别不用配置，不画「服务配置」。
test("the system recognizer has no service configuration row", async () => {
  renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "zh-CN", asr_provider: "system" } },
  });
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "服务配置" })).toBeNull();
});

test("the phone voice page picks the service and the stream endpoint from sheets", async () => {
  const { save } = renderSettings({
    page: "voice",
    preferences: {
      voice_input: {
        enabled: true,
        language: "",
        asr_provider: "doubao",
        asr_endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
      },
    },
  });
  await settingsFormReady();
  // 两行原本是行尾的原生下拉框，手机上会把说明挤成几个字一行；现在和其他选择行一样点开面板。
  expect(within(openSheet("识别服务")).getByRole("button", { name: "豆包" })).toBeTruthy();
  fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
  openServiceConfig();
  expect(shownValue("流式接口").textContent).toContain("双向流式（增量结果）");
  fireEvent.click(
    within(openSheet("流式接口")).getByRole("button", { name: "整句流式（准确率更高）" }),
  );
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].voice_input.asr_endpoint).toBe(
    "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream",
  );
});

test("the phone voice page lays credentials and devices out as rows of their groups", async () => {
  renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "", asr_provider: "doubao" } },
    devices: true,
  });
  await settingsFormReady();
  openServiceConfig();
  const service = screen.getByRole("region", { name: "识别服务" });
  // API Key 原本是旧的 `label.section-header` 字段，放在分组里缺了行的内边距，贴着卡片左边。
  expect(within(service).getByText("Doubao API Key").hasAttribute("data-row-title")).toBe(true);
  expect(within(service).getByLabelText("Doubao API Key")).toBeTruthy();
  // HarmonyOS 只有一种录音方式：一个「麦克风」选择行，打开子视图时自动读取设备，不再有刷新按钮和「可用录音设备」。
  const devices = screen.getByRole("region", { name: "录音设备" });
  expect(within(devices).queryByRole("button", { name: "刷新设备" })).toBeNull();
  expect(within(devices).queryByText("可用录音设备")).toBeNull();
  expect(
    within(devices)
      .getAllByRole("button")
      .some(
        (button) =>
          button.getAttribute("aria-haspopup") === "dialog" &&
          button.querySelector("[data-row-title]")?.textContent === "麦克风",
      ),
  ).toBe(true);
  // 选中的麦克风拔掉后 HarmonyOS 改用系统默认设备，说明照实写，不说「无法录音」。
  expect(within(devices).getByText(/拔掉后改用系统默认设备/)).toBeTruthy();
  expect(within(devices).queryByText(/拔掉后无法录音/)).toBeNull();
});

test("the settings scroller contains the hidden controls behind picker rows", async () => {
  renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  // 选择行背后的原生控件是 `sr-only`（绝对定位）。滚动容器不是定位元素时，它们相对文档定位、把文档撑得比窗口高，长页面滚到底再上滑会把整个应用滑出屏幕。jsdom 不做布局，只能检查容器是定位元素。
  const scroller = screen.getByRole("group", { name: "语音输入" }).closest("main");
  expect(scroller?.classList.contains("relative")).toBe(true);
});

test("automatic punctuation is Doubao's and is greyed out with the reason elsewhere", async () => {
  const { save } = renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  const recognition = screen.getByRole("region", { name: "识别" });
  const punctuation = within(recognition).getByRole("switch", {
    name: "自动添加标点",
  }) as HTMLInputElement;
  expect(punctuation.checked).toBe(true);
  expect(punctuation.disabled).toBe(false);
  fireEvent.click(punctuation);
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].voice_input.doubao_enable_punc).toBe(false);

  cleanup();
  renderSettings({
    page: "voice",
    preferences: { voice_input: { enabled: true, language: "", asr_provider: "openai" } },
  });
  await settingsFormReady();
  const other = within(screen.getByRole("region", { name: "识别" })).getByRole("switch", {
    name: "自动添加标点",
  }) as HTMLInputElement;
  expect(other.disabled).toBe(true);
  expect(screen.getByText("当前识别服务不支持，仅豆包语音识别可以自动加标点")).toBeTruthy();
});

test("the polish scheme sheet keeps the custom slots and shows the stored one", async () => {
  const { save } = renderSettings({
    page: "voice",
    // AI 润色只在 AI 生成类资质到位、总闸打开（`ai_generation`）时提供。
    aiGeneration: true,
    preferences: {
      voice_input: {
        enabled: true,
        language: "",
        asr_provider: "doubao",
        polish_enabled: true,
        polish_text: true,
        polish_prompt_id: "custom_2",
      },
    },
  });
  await settingsFormReady();
  openServiceConfig();
  // 存的是自定义槽位时，行上显示该槽位，而不是回退到第一个预设。
  expect(shownValue("润色方案").textContent).toContain("自定义二");
  const sheet = openSheet("润色方案");
  for (const label of ["精炼整理", "自定义一", "自定义二", "自定义三"]) {
    expect(within(sheet).getByRole("button", { name: label })).toBeTruthy();
  }
  fireEvent.click(within(sheet).getByRole("button", { name: "自定义三" }));
  expect(shownValue("润色方案").textContent).toContain("自定义三");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].voice_input.polish_prompt_id).toBe("custom_3");
});

// 鸿蒙版拿到 AI 生成类资质之前（`ai_generation` 为 false）：存下的润色开关打开了，服务配置里也没有「文本润色」这一组，手机和 2in1 都一样。
test.each([true, false])(
  "HarmonyOS without AI generation has no AI polish group (phone: %s)",
  async (mobile) => {
    renderSettings({
      page: "voice",
      mobile,
      preferences: {
        voice_input: {
          enabled: true,
          language: "",
          asr_provider: "doubao",
          polish_enabled: true,
          polish_text: true,
        },
      },
    });
    await settingsFormReady();
    fireEvent.click(screen.getByRole("button", { name: /^服务配置/ }));
    // 识别服务照常在，只是没有润色。
    expect((await screen.findAllByText("识别服务")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("region", { name: "文本润色" })).toBeNull();
    expect(screen.queryByText("启用润色")).toBeNull();
    expect(screen.queryByText("润色方案")).toBeNull();
  },
);

test("the 2-in-1 keeps the desktop voice page", async () => {
  renderSettings({
    page: "voice",
    mobile: false,
    preferences: { voice_input: { enabled: true, language: "zh-cn", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  expect(screen.getByText("HarmonyOS 输入法语音")).toBeTruthy();
  // 桌面语音页和手机一样把豆包的标点开关放在「识别」组里，密钥和接口收进「服务配置」子视图。
  expect(screen.getByRole("switch", { name: "自动添加标点" })).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: "Doubao API Key" })).toBeNull();
  // 豆包没填凭据时 HarmonyOS 改用系统识别，识别语言照旧有用，所以 2in1 上不随豆包隐藏；状态也按键盘的同一套判断写。
  expect((screen.getByLabelText("识别语言") as HTMLInputElement).tagName).toBe("INPUT");
  expect(screen.getByRole("button", { name: "服务配置" }).textContent).toContain(
    "未填写，暂用系统识别",
  );
});

// ---- 手写输入 ----

test("the phone handwriting page offers the pad's delay, ink colour and width", async () => {
  const { saveFeedback } = renderSettings({ page: "handwriting", feedback: padFeedback });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "手写输入" });
  await waitFor(() => expect(groupTitles(page)).toEqual(["预览", "书写", "笔迹"]));
  // 预览画的是手写板：用皮肤的颜色写「水」，笔迹按当前的颜色和粗细画。
  const preview = page.querySelector("[data-handwriting-preview]");
  expect(preview?.querySelectorAll("path")).toHaveLength(4);
  const delay = within(page).getByRole("slider", { name: "识别等待时间" }) as HTMLInputElement;
  expect([delay.min, delay.max, delay.step, delay.value]).toEqual(["200", "1500", "100", "600"]);
  expect(within(page).getByText("600ms")).toBeTruthy();
  const width = within(page).getByRole("slider", { name: "笔迹粗细" }) as HTMLInputElement;
  expect([width.min, width.max, width.step, width.value]).toEqual(["1", "8", "1", "3"]);
  expect(within(page).getByText("3px")).toBeTruthy();
  expect(shownValue("笔迹颜色").textContent).toContain("跟随皮肤");
  // 隐私说明是「笔迹」组的解释，收进组名旁的「?」，不常驻在行之外。
  const privacy = within(page).getByText(/手写使用系统的文字识别能力/);
  expect(privacy.closest("[data-group-header]")).not.toBeNull();
  // 与 Android 的手写页一样不附启用说明和系统设置按钮，那是首页状态卡片的事。
  expect(within(page).queryByText(/方案选择器/)).toBeNull();
  expect(within(page).queryByRole("button", { name: "打开系统输入法设置" })).toBeNull();

  fireEvent.click(within(openSheet("笔迹颜色")).getByRole("button", { name: "蓝色" }));
  await waitFor(() => expect(saveFeedback).toHaveBeenCalledTimes(1));
  expect(saveFeedback.mock.calls[0][0]).toEqual({ ...padFeedback, handwritingStrokeColor: "blue" });

  // 拖动只保存一次，保存的是停下时的值。
  fireEvent.change(delay, { target: { value: "800" } });
  fireEvent.change(delay, { target: { value: "900" } });
  expect(within(page).getByText("900ms")).toBeTruthy();
  await waitFor(() => expect(saveFeedback).toHaveBeenCalledTimes(2));
  expect(saveFeedback.mock.calls[1][0]).toMatchObject({
    handwritingStrokeColor: "blue",
    handwritingDelayMs: 900,
  });
  fireEvent.change(width, { target: { value: "5" } });
  await waitFor(() => expect(saveFeedback).toHaveBeenCalledTimes(3));
  expect(saveFeedback.mock.calls[2][0]).toMatchObject({
    handwritingDelayMs: 900,
    handwritingStrokeWidth: 5,
  });
  expect(within(page).getByText("5px")).toBeTruthy();
});

test("a keyboard without pad settings leaves the handwriting page as instructions", async () => {
  const withoutPad: MobileKeyboardFeedback = {
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "medium",
  };
  renderSettings({ page: "handwriting", feedback: withoutPad });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "手写输入" });
  expect(groupTitles(page)).toEqual(["HarmonyOS 键盘手写"]);
  expect(within(page).queryByRole("slider")).toBeNull();
  expect(
    within(page)
      .getByText(/手写使用系统的文字识别能力/)
      .closest("section"),
  ).not.toBeNull();
});

// 2-in-1 的手写只是一段说明，并进「屏幕键盘与手写」页；旧的 handwriting 路由落到合并后的页。
test("the 2-in-1 handwriting instructions sit on the merged screen keyboard page", async () => {
  renderSettings({ page: "handwriting", mobile: false, feedback: padFeedback });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "键盘与手写" });
  expect(screen.queryByRole("group", { name: "手写输入" })).toBeNull();
  expect(within(page).getByRole("tab", { name: "键盘", selected: true })).toBeTruthy();
  expect(within(page).queryByRole("slider", { name: "识别等待时间" })).toBeNull();
  fireEvent.click(within(page).getByRole("tab", { name: "手写" }));
  expect(within(page).getByText(/2-in-1 候选窗口不绘制键面/)).toBeTruthy();
  const handwriting = within(page).getByRole("tabpanel", { name: "手写" });
  expect(groupTitles(handwriting)).toEqual(["HarmonyOS 键盘手写"]);
});

// ---- 键盘 ----

test("the phone keyboard page opens with 布局 and its Chinese keyboard choice", async () => {
  const { save } = renderSettings({ page: "screen-keyboard", feedback: padFeedback });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "屏幕键盘" });
  await waitFor(() => expect(groupTitles(page)).toContain("按键反馈"));
  const titles = groupTitles(page);
  // 「布局」只放键盘的选择；预览、三个滑块和「恢复默认」在同一张「尺寸」卡片里，预览在最上面。
  expect(titles.slice(0, 2)).toEqual(["布局", "尺寸"]);
  expect(titles).not.toContain("屏幕键盘");
  const layout = within(page).getByRole("region", { name: "布局" });
  expect(within(layout).getAllByText("中文键盘").length).toBeGreaterThan(0);
  expect(within(layout).queryByText("键盘高度")).toBeNull();
  const size = within(page).getByRole("region", { name: "尺寸" });
  for (const row of ["键盘高度", "按键间距", "行间距"]) {
    expect(within(size).getAllByText(row).length).toBeGreaterThan(0);
  }
  expect(within(size).getByRole("button", { name: "恢复屏幕键盘默认设置" }).textContent).toBe(
    "恢复默认",
  );
  const preview = within(size).getByRole("group", { name: "屏幕键盘预览" });
  expect(within(preview).getByText(/在预览上左右拖动/)).toBeTruthy();
  // 预览画成和真键盘一样的手机键盘：顶上是工具栏图标，不是一行「水杉 IME」；键距按设置的 dp 画。
  expect(preview.querySelector("[data-keyboard-toolbar]")).toBeTruthy();
  expect(within(preview).queryByText("水杉 IME")).toBeNull();
  expect(preview.querySelector("svg")?.getAttribute("viewBox")).toMatch(/^0 0 390 /);
  // 手机上的行只留标题和取值，没有长说明。
  expect(within(page).queryByText(/高度和间距只改变触屏键位的外观/)).toBeNull();
  // 26 键按 123 固定是数字一行、九宫格数字层固定 123 在上，这两项不再可设置。
  expect(within(page).queryByText("26 键按 123")).toBeNull();
  expect(within(page).queryByText("九宫格数字顺序")).toBeNull();
  // 高度为 0 时显示「标准」，不显示 dp。
  expect(within(size).getByText("标准")).toBeTruthy();
  // HarmonyOS 键盘的工具栏上没有语音按钮可供这个开关显示。
  expect(within(page).queryByRole("switch", { name: "顶部语音入口" })).toBeNull();
  expect(shownValue("中文键盘").textContent).toContain("26 键");
  // 预览画的是当前的中文键盘：26 键是字母键，切到 9 键后是九宫格（左列标点、ABC…WXYZ、右列分词），不再是 26 个字母。
  expect(within(preview).getByText("q")).toBeTruthy();
  expect(within(preview).queryByText("ABC")).toBeNull();

  fireEvent.click(within(openSheet("中文键盘")).getByRole("button", { name: "9 键" }));
  expect(shownValue("中文键盘").textContent).toContain("9 键");
  for (const label of ["ABC", "WXYZ", "分词", "@#"]) {
    expect(within(preview).getByText(label)).toBeTruthy();
  }
  expect(within(preview).queryByText("q")).toBeNull();
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.scheme).toBe("quanpin");
  expect(saved.touch_keyboard_layout).toBe("nine_key");
  expect(saved.touch_keyboard_schemes.selected).toBe("nine_key");
  expect(saved.touch_keyboard_schemes.enabled).toContain("nine_key");
});

test("the Chinese keyboard row offers 26, 14, 18 and 9 keys for 全拼 and 26 and 9 for 日语", async () => {
  const { save } = renderSettings({ page: "screen-keyboard", feedback: padFeedback });
  await settingsFormReady();
  const quanpin = openSheet("中文键盘");
  expect(
    within(quanpin)
      .getAllByRole("button")
      .filter((button) => button.hasAttribute("data-sheet-option"))
      .map((button) => button.textContent),
  ).toEqual(["26 键", "14 键", "18 键", "9 键"]);
  fireEvent.click(within(quanpin).getByRole("button", { name: "14 键" }));
  expect(shownValue("中文键盘").textContent).toContain("14 键");
  const preview = screen.getByRole("group", { name: "屏幕键盘预览" });
  expect(within(preview).getByText("QW")).toBeTruthy();
  expect(within(preview).queryByText("q")).toBeNull();
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.scheme).toBe("quanpin");
  expect(saved.touch_keyboard_layout).toBe("fourteen_key");
  expect(saved.touch_keyboard_schemes.selected).toBe("fourteen_key");
  expect(saved.touch_keyboard_schemes.enabled).toContain("fourteen_key");
  cleanup();

  renderSettings({
    page: "screen-keyboard",
    preferences: { scheme: "japanese", touch_keyboard_layout: "twenty_six_key" },
  });
  await settingsFormReady();
  expect(
    within(openSheet("中文键盘"))
      .getAllByRole("button")
      .filter((button) => button.hasAttribute("data-sheet-option"))
      .map((button) => button.textContent),
  ).toEqual(["26 键", "9 键"]);
});

test("a scheme with one keyboard says where to change it", async () => {
  renderSettings({
    page: "screen-keyboard",
    preferences: { scheme: "korean", touch_keyboard_layout: "twenty_six_key" },
  });
  await settingsFormReady();
  const row = shownValue("中文键盘") as HTMLButtonElement;
  expect(row.disabled).toBe(true);
  expect(row.textContent).toContain("当前方案只有一种键盘，在「输入」里换方案");
});

// 五笔的 26 / 14 键只写 `touch_wubi_layout`：方案、86 / 98 版和启用列表都不动，预览画 14 键键面。
test("the Wubi keyboard row offers 26 and 14 keys and writes only touch_wubi_layout", async () => {
  const { save } = renderSettings({
    page: "screen-keyboard",
    feedback: padFeedback,
    preferences: {
      scheme: "wubi",
      wubi_profile: "wubi98",
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes: { enabled: ["quanpin", "wubi"], selected: "wubi" },
    },
  });
  await settingsFormReady();
  expect(screen.queryByText("当前方案只有一种键盘，在「输入」里换方案")).toBeNull();
  expect(shownValue("五笔键盘").textContent).toContain("26 键");
  const preview = screen.getByRole("group", { name: "屏幕键盘预览" });
  expect(within(preview).getByText("q")).toBeTruthy();
  const sheet = openSheet("五笔键盘");
  expect(
    within(sheet)
      .getAllByRole("button")
      .filter((button) => button.hasAttribute("data-sheet-option"))
      .map((button) => button.textContent),
  ).toEqual(["26 键", "14 键"]);
  fireEvent.click(within(sheet).getByRole("button", { name: "14 键" }));
  expect(shownValue("五笔键盘").textContent).toContain("14 键");
  for (const label of ["QW", "ZX", "L", "符"]) {
    expect(within(preview).getByText(label)).toBeTruthy();
  }
  expect(within(preview).queryByText("L;")).toBeNull();
  expect(within(preview).queryByText("q")).toBeNull();
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.touch_wubi_layout).toBe("fourteen_key");
  expect(saved.scheme).toBe("wubi");
  expect(saved.wubi_profile).toBe("wubi98");
  expect(saved.touch_keyboard_layout).toBe("twenty_six_key");
  expect(saved.touch_keyboard_schemes).toEqual({ enabled: ["quanpin", "wubi"], selected: "wubi" });
  expect(saved.touch_shuangpin_layout).toBeUndefined();
});

test("a saved Wubi 14-key layout is shown and previewed, and quanpin ignores it", async () => {
  renderSettings({
    page: "screen-keyboard",
    preferences: {
      scheme: "wubi",
      touch_keyboard_layout: "twenty_six_key",
      touch_wubi_layout: "fourteen_key",
    },
  });
  await settingsFormReady();
  expect(shownValue("五笔键盘").textContent).toContain("14 键");
  const preview = screen.getByRole("group", { name: "屏幕键盘预览" });
  expect(within(preview).getByText("QW")).toBeTruthy();
  cleanup();

  renderSettings({
    page: "screen-keyboard",
    preferences: {
      scheme: "quanpin",
      touch_keyboard_layout: "twenty_six_key",
      touch_wubi_layout: "fourteen_key",
    },
  });
  await settingsFormReady();
  expect(screen.queryByText("五笔键盘")).toBeNull();
  expect(shownValue("中文键盘").textContent).toContain("26 键");
  expect(within(screen.getByRole("group", { name: "屏幕键盘预览" })).getByText("q")).toBeTruthy();
});

test("the phone keyboard page edits which toolbar tools are shown and whether the row is hidden", async () => {
  const { save } = renderSettings({
    page: "screen-keyboard",
    feedback: padFeedback,
    preferences: { harmony_toolbar: { clipboard: false } },
  });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "屏幕键盘" });
  const toolbar = within(page).getByRole("region", { name: "工具栏" });
  const switches = ["表情", "常用语", "剪贴板", "皮肤", "输入方式"].map(
    (label) =>
      within(toolbar).getByRole("switch", { name: `工具栏：${label}` }) as HTMLInputElement,
  );
  expect(switches.map((item) => item.checked)).toEqual([true, true, false, true, true]);
  expect(shownValue("显示方式").textContent).toContain("输入时显示");
  // 「恢复默认」也把工具栏设置恢复，说明里因此列出它。
  expect(within(page).getByRole("button", { name: "恢复屏幕键盘默认设置" })).toBeTruthy();

  fireEvent.click(switches[1]);
  fireEvent.click(within(openSheet("显示方式")).getByRole("button", { name: "隐藏" }));
  expect(shownValue("显示方式").textContent).toContain("隐藏");
  expect(within(toolbar).getByText(/工具栏已隐藏/)).toBeTruthy();
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].harmony_toolbar).toEqual({
    emoji: true,
    phrase: false,
    clipboard: false,
    skin: true,
    scheme: true,
    hidden: true,
  });
});

test("the 2-in-1 keyboard page has no toolbar tools, as its keyboard has no toolbar row", async () => {
  renderSettings({ page: "screen-keyboard", mobile: false });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "键盘与手写" });
  expect(within(page).queryByRole("switch", { name: "工具栏：表情" })).toBeNull();
});

test("the 2-in-1 keyboard page keeps the preview above 尺寸", async () => {
  renderSettings({ page: "screen-keyboard", mobile: false });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "键盘与手写" });
  expect(groupTitles(page).slice(0, 2)).toEqual(["屏幕键盘", "尺寸"]);
  expect(within(page).queryByText("中文键盘")).toBeNull();
});

test("the 2-in-1 keyboard page offers no Wubi 14-key layout", async () => {
  renderSettings({
    page: "screen-keyboard",
    mobile: false,
    preferences: { scheme: "wubi", touch_wubi_layout: "fourteen_key" },
  });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "键盘与手写" });
  expect(within(page).queryByText("五笔键盘")).toBeNull();
  expect(within(page).queryByText("QW")).toBeNull();
});
