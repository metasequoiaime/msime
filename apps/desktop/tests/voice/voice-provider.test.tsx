// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";

afterEach(cleanup);

const base: Snapshot = {
  format_version: 1,
  revision: 4,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function mount(client: Partial<SettingsClient>) {
  return render(<SettingsPage client={{ load: async () => base, save: vi.fn(), ...client }} />);
}

async function openVoice() {
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  return screen.getByLabelText("识别服务") as HTMLSelectElement;
}

// The Linux voice provider builds ASR_PROVIDERS = {openai, groq, siliconflow, everyapi,
// mistral, doubao}. Anything else makes select_profile return None and every recording fail
// with ok=false. Keep this list in step with that script, the shared C++ provider table and
// the mobile transport's accepted set; an option no backend implements is a dead menu entry.
const REACHABLE = ["doubao", "siliconflow", "openai", "groq", "everyapi", "mistral"];

test("macOS system recognition is configurable without cloud ASR fields", async () => {
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...base, revision: 5, preferences }));
  mount({
    save,
    host: testHost({ platform: "macos" }),
    load: async () => ({
      ...base,
      preferences: {
        ...base.preferences,
        voice_input: {
          enabled: true,
          language: "auto",
          asr_provider: "openai",
          asr_token: "synthetic-only",
          asr_endpoint: "https://api.openai.com/v1/audio/transcriptions",
          asr_model: "whisper-1",
        },
      },
    }),
  });
  const select = await openVoice();
  expect(Array.from(select.options).map((option) => option.value)).toContain("system");
  fireEvent.change(select, { target: { value: "system" } });
  expect(screen.getByText(/首次使用需授予麦克风和语音识别权限/)).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: "识别接口地址" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "识别模型" })).toBeNull();
  expect(screen.queryByLabelText("识别 API Token")).toBeNull();
  expect(screen.queryByRole("textbox", { name: "Doubao 资源 ID" })).toBeNull();
  fireEvent.change(screen.getByRole("combobox", { name: "结果提交策略" }), {
    target: { value: "ctrl_v" },
  });
  expect(screen.getByRole("switch", { name: "启用文本润色" })).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "识别语言" }) as HTMLInputElement).value).toBe(
    "zh-CN",
  );
  saveSettingsNow();
  await vi.waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls[0][1].voice_input).toMatchObject({
    asr_provider: "system",
    commit_mode: "ctrl_v",
    asr_token: "",
    asr_endpoint: "",
    asr_model: "",
    asr_tokens: { openai: "synthetic-only" },
  });
});

test("a stored system provider is preserved on macOS and marked unavailable elsewhere", async () => {
  for (const platform of ["macos", "windows", "linux"]) {
    mount({
      host: testHost({ platform }),
      load: async () => ({
        ...base,
        preferences: {
          ...base.preferences,
          voice_input: { enabled: true, language: "zh-CN", asr_provider: "system" },
        },
      }),
    });
    const select = await openVoice();
    expect(select.value).toBe("system");
    expect(Array.from(select.options).find((option) => option.value === "system")?.disabled).toBe(
      platform !== "macos",
    );
    cleanup();
  }
});

test("the recognition dropdown only offers providers a backend implements", async () => {
  mount({});
  const select = await openVoice();
  const offered = Array.from(select.options).map((option) => option.value);
  expect(offered.sort()).toEqual([...REACHABLE].sort());
  // These two shipped for a while and no backend has ever implemented them.
  expect(offered).not.toContain("local_whisper");
  expect(offered).not.toContain("cloud");
});

test("the default recognition provider is one the dropdown offers", async () => {
  mount({});
  const select = await openVoice();
  expect(REACHABLE).toContain(select.value);
  // A default outside the option list would render as an empty selection.
  expect(select.value).not.toBe("");
});

test("a stored provider selection round-trips through save", async () => {
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...base, revision: 5, preferences }));
  mount({ save });
  const select = await openVoice();
  fireEvent.change(select, { target: { value: "siliconflow" } });
  saveSettingsNow();
  await vi.waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls[0][1].voice_input.asr_provider).toBe("siliconflow");
});
