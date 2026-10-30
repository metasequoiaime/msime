// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";
import { settingsCapabilities } from "../../../../packages/ui/src/settings/settings-capabilities";

// 鸿蒙版拿到 AI 生成类资质之前，宿主报 `ai_generation` 为 false（`crates/client-core/src/host_surface.rs`），设置页据此不给 AI 辅助页、AI 对话和 AI 皮肤；其他宿主不变。

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    // 从别的设备同步来的、打开了 AI 的偏好：页面照样不给入口。
    ai_assistant: {
      enabled: true,
      provider: "deepseek",
      model: "deepseek-chat",
      endpoint: "https://api.deepseek.com/v1",
      candidate_limit: 3,
      prompt_custom_1: "",
      prompt_custom_2: "",
      prompt_custom_3: "",
    },
    translation_ai: true,
  },
} as Snapshot;

function aiClients(): Partial<SettingsClient> {
  return {
    aiAssistant: { fetchModels: vi.fn(), test: vi.fn() },
    chat: {
      models: vi.fn().mockResolvedValue({ data: [{ id: "fixture" }], defaultModel: "fixture" }),
      complete: vi.fn(),
    },
  };
}

function renderDesktop(
  platform: string,
  initialPage = "input",
  fields: Record<string, unknown> = {},
) {
  const client = aiClients();
  render(
    <SettingsPage
      initialPage={initialPage}
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        // 鸿蒙 2in1 用桌面形态的设置页：侧栏，没有底部标签。
        host: testHost({ platform, mobile_settings: false, panel_windows: true, ...fields }),
        ...client,
      }}
    />,
  );
  return client;
}

test("only an explicit false from the host withholds AI generation", () => {
  const flags = {
    linux: false,
    android: false,
    ios: false,
    harmony: true,
    windows: false,
    macos: false,
    mobile: true,
    canRestartInputMethod: false,
    canInstallInputSource: false,
    canListVoiceCaptureDevices: false,
  };
  expect(
    settingsCapabilities({ ...flags, host: testHost({ platform: "harmony" }) }).showAiGeneration,
  ).toBe(false);
  expect(
    settingsCapabilities({ ...flags, host: testHost({ platform: "harmony", ai_generation: true }) })
      .showAiGeneration,
  ).toBe(true);
  // 早于这一项的宿主不报它，网页预览没有宿主：都照旧给出。
  const { ai_generation: _omitted, ...older } = testHost({ platform: "macos" });
  expect(
    settingsCapabilities({ ...flags, harmony: false, macos: true, host: older }).showAiGeneration,
  ).toBe(true);
  expect(settingsCapabilities({ ...flags, host: undefined }).showAiGeneration).toBe(true);
  for (const platform of ["macos", "windows", "linux", "android", "ios"]) {
    expect(testHost({ platform }).ai_generation).toBe(true);
  }
});

test("the HarmonyOS 2in1 sidebar has no AI page and its routes fall back", async () => {
  const client = renderDesktop("harmony", "ai");
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: /^(AI|AI 辅助)$/ })).toBeNull();
  expect(screen.queryByRole("group", { name: "AI 辅助" })).toBeNull();
  expect(screen.queryByRole("region", { name: "AI 对话" })).toBeNull();
  // 页面不发模型列表、润色测试和对话请求。
  expect(client.aiAssistant!.fetchModels).not.toHaveBeenCalled();
  expect(client.aiAssistant!.test).not.toHaveBeenCalled();
  expect(client.chat!.models).not.toHaveBeenCalled();
});

test("the HarmonyOS 2in1 does not open the chat by route", async () => {
  const client = renderDesktop("harmony", "chat");
  await settingsFormReady();
  expect(screen.queryByRole("region", { name: "AI 对话" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "聊天消息" })).toBeNull();
  expect(client.chat!.models).not.toHaveBeenCalled();
});

test.each(["macos", "windows", "linux"])("%s keeps its AI page", async (platform) => {
  renderDesktop(platform, "ai");
  await settingsFormReady();
  expect(screen.getByRole("button", { name: /^(AI|AI 辅助)$/ })).toBeTruthy();
  expect(screen.getByRole("group", { name: "AI 辅助" })).toBeTruthy();
});

test("HarmonyOS keeps its AI page once the host reports AI generation again", async () => {
  renderDesktop("harmony", "ai", { ai_generation: true });
  await settingsFormReady();
  expect(screen.getByRole("group", { name: "AI 辅助" })).toBeTruthy();
});

test("the HarmonyOS statistics page draws no AI polish or reply category", async () => {
  const today = new Date();
  const day = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
  const sources = { quanpin: 6, ai: 4, reply: 2 };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue({
      statistics: {
        enabled: true,
        total: 12,
        days: { [day]: 12 },
        detail: { characters: { han: 12 }, sources },
        dailyDetails: { [day]: { characters: { han: 12 }, sources } },
      },
      availability: "ready",
      lastWrittenMs: Date.now(),
    }),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      initialPage="typing-statistics"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: false, panel_windows: true }),
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "方案" }));
  expect(screen.getByLabelText(/^全拼 26 键 6 字符/)).toBeTruthy();
  expect(screen.queryByLabelText(/^AI 润色/)).toBeNull();
  expect(screen.queryByLabelText(/^高情商回复/)).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "模式" }));
  expect(screen.queryByLabelText(/^AI 润色/)).toBeNull();
  expect(screen.queryByText(/AI 润色和语音输入/)).toBeNull();
});
