// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type MobileKeyboardFeedback, type Snapshot } from "@msime/ui";

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
}: {
  page: string;
  preferences?: Record<string, unknown>;
  mobile?: boolean;
  feedback?: MobileKeyboardFeedback;
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
        }),
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

// ---- 语音输入 ----

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
  expect(within(page).getByText(/豆包配置有效时，键盘直接采集 16 kHz 麦克风音频/)).toBeTruthy();
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

test("the 2-in-1 keeps the desktop voice page", async () => {
  renderSettings({
    page: "voice",
    mobile: false,
    preferences: { voice_input: { enabled: true, language: "zh-cn", asr_provider: "doubao" } },
  });
  await settingsFormReady();
  expect(screen.getByText("HarmonyOS 输入法语音")).toBeTruthy();
  expect(screen.queryByText("自动添加标点")).toBeNull();
  expect((screen.getByLabelText("识别语言") as HTMLInputElement).tagName).toBe("INPUT");
});

// ---- 手写输入 ----

test("the phone handwriting page offers the pad's delay, ink colour and width", async () => {
  const { saveFeedback } = renderSettings({ page: "handwriting", feedback: padFeedback });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "手写输入" });
  await waitFor(() => expect(groupTitles(page)).toEqual(["书写", "笔迹", "HarmonyOS 键盘手写"]));
  const delay = within(page).getByRole("slider", { name: "识别等待时间" }) as HTMLInputElement;
  expect([delay.min, delay.max, delay.step, delay.value]).toEqual(["200", "1500", "100", "600"]);
  expect(within(page).getByText("600ms")).toBeTruthy();
  const width = within(page).getByRole("slider", { name: "笔迹粗细" }) as HTMLInputElement;
  expect([width.min, width.max, width.step, width.value]).toEqual(["1", "8", "1", "3"]);
  expect(within(page).getByText("3px")).toBeTruthy();
  expect(shownValue("笔迹颜色").textContent).toContain("跟随皮肤");
  // 隐私说明是「笔迹」的页脚，在该组的行之外。
  const privacy = within(page).getByText(/手写使用系统的文字识别能力/);
  expect(privacy.closest("section")).toBeNull();
  // 启用说明和系统设置按钮仍然存在。
  expect(within(page).getByText(/再从键盘的方案选择器切换到“手写”/)).toBeTruthy();
  expect(within(page).getByRole("button", { name: "打开系统输入法设置" })).toBeTruthy();

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

test("the 2-in-1 handwriting page draws no pad settings", async () => {
  renderSettings({ page: "handwriting", mobile: false, feedback: padFeedback });
  await settingsFormReady();
  expect(screen.queryByRole("slider", { name: "识别等待时间" })).toBeNull();
  expect(screen.getByText(/2-in-1 候选窗口不绘制键面/)).toBeTruthy();
});

// ---- 键盘 ----

test("the phone keyboard page opens with 布局 and its Chinese keyboard choice", async () => {
  const { save } = renderSettings({ page: "screen-keyboard", feedback: padFeedback });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "屏幕键盘" });
  await waitFor(() => expect(groupTitles(page)).toContain("按键反馈"));
  const titles = groupTitles(page);
  expect(titles.slice(0, 2)).toEqual(["布局", "屏幕键盘"]);
  expect(titles).not.toContain("尺寸");
  const layout = within(page).getByRole("region", { name: "布局" });
  for (const row of ["中文键盘", "键盘高度", "按键间距", "行间距", "恢复默认"]) {
    expect(within(layout).getAllByText(row).length).toBeGreaterThan(0);
  }
  expect(within(layout).getByText(/下方预览/)).toBeTruthy();
  // 预览跟随它所展示的控件。
  expect(
    within(within(page).getByRole("region", { name: "屏幕键盘" })).getByLabelText("屏幕键盘预览"),
  ).toBeTruthy();
  // HarmonyOS 键盘的工具栏上没有语音按钮可供这个开关显示。
  expect(within(page).queryByRole("switch", { name: "顶部语音入口" })).toBeNull();
  expect(shownValue("中文键盘").textContent).toContain("26 键");

  fireEvent.click(within(openSheet("中文键盘")).getByRole("button", { name: "9 键" }));
  expect(shownValue("中文键盘").textContent).toContain("9 键");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.scheme).toBe("quanpin");
  expect(saved.touch_keyboard_layout).toBe("nine_key");
  expect(saved.touch_keyboard_schemes.selected).toBe("nine_key");
  expect(saved.touch_keyboard_schemes.enabled).toContain("nine_key");
});

test("a scheme with one keyboard says where to change it", async () => {
  renderSettings({
    page: "screen-keyboard",
    preferences: { scheme: "shuangpin", touch_keyboard_layout: "twenty_six_key" },
  });
  await settingsFormReady();
  const row = shownValue("中文键盘") as HTMLButtonElement;
  expect(row.disabled).toBe(true);
  expect(row.textContent).toContain("当前方案只有一种键盘，在「输入」里换方案");
});

test("the 2-in-1 keyboard page keeps the preview above 尺寸", async () => {
  renderSettings({ page: "screen-keyboard", mobile: false });
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "屏幕键盘" });
  expect(groupTitles(page).slice(0, 2)).toEqual(["屏幕键盘", "尺寸"]);
  expect(within(page).queryByText("中文键盘")).toBeNull();
});
