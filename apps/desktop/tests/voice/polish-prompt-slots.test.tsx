// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  POLISH_PRESETS,
  POLISH_PRESET_IDS,
  SettingsPage,
  isPolishCustomSlot,
  polishPromptFor,
  polishSlotField,
  polishPresetPrompt,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    voice_input: {
      enabled: true,
      language: "zh-CN",
      asr_provider: "doubao",
      polish_enabled: true,
      polish_prompt_id: "cleanup",
      polish_prompt_custom_2: "我自己的方案",
    },
  },
};

test("every preset carries the real multi-rule prompt, not a one-liner", () => {
  for (const id of POLISH_PRESET_IDS) {
    const prompt = POLISH_PRESETS[id];
    // The shipped host's prompts are 5-8 rules; the client used to paraphrase
    // them into a single sentence, which is a materially weaker instruction.
    expect(prompt.length).toBeGreaterThan(100);
    expect(prompt).toContain("<asr_text>");
    expect(prompt.split("\n").length).toBeGreaterThan(3);
  }
  // Each preset must be distinct, or choosing one would be meaningless.
  expect(new Set(Object.values(POLISH_PRESETS)).size).toBe(POLISH_PRESET_IDS.length);
});

test("only the numbered slots are custom slots", () => {
  expect(isPolishCustomSlot("custom_1")).toBe(true);
  expect(isPolishCustomSlot("custom")).toBe(false);
  expect(isPolishCustomSlot("zh2en")).toBe(false);
  expect(polishPresetPrompt("custom_1")).toBe("");
  expect(polishPresetPrompt("zh2en")).toBe(POLISH_PRESETS.zh2en);
});

test("resolves preset and custom prompt slots through shared helpers", () => {
  expect(polishSlotField("custom_2")).toBe("polish_prompt_custom_2");
  expect(polishSlotField("cleanup")).toBeUndefined();
  expect(polishPromptFor("zh2en", {})).toBe(POLISH_PRESETS.zh2en);
  expect(polishPromptFor("custom_2", { polish_prompt_custom_2: "合成提示词" })).toBe("合成提示词");
});

async function openVoice() {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  return await screen.findByLabelText("润色方案");
}

test("choosing a preset loads its actual text into the box", async () => {
  const select = await openVoice();
  const box = () => screen.getByLabelText("润色提示词") as HTMLTextAreaElement;
  // Before this, picking a preset stored an id with nothing behind it and left
  // the textarea showing something unrelated.
  fireEvent.change(select, { target: { value: "zh2en" } });
  expect(box().value).toBe(POLISH_PRESETS.zh2en);
  fireEvent.change(select, { target: { value: "casual" } });
  expect(box().value).toBe(POLISH_PRESETS.casual);
});

test("a custom slot shows what the user stored in that slot", async () => {
  const select = await openVoice();
  fireEvent.change(select, { target: { value: "custom_2" } });
  expect((screen.getByLabelText("润色提示词") as HTMLTextAreaElement).value).toBe("我自己的方案");
  // An empty slot is empty, not the preset text.
  fireEvent.change(select, { target: { value: "custom_3" } });
  expect((screen.getByLabelText("润色提示词") as HTMLTextAreaElement).value).toBe("");
});

test("a preset prompt is read-only and editing a custom slot saves to that slot", async () => {
  const select = await openVoice();
  fireEvent.change(select, { target: { value: "faithful" } });
  expect((screen.getByLabelText("润色提示词") as HTMLTextAreaElement).readOnly).toBe(true);
  expect(screen.queryByRole("button", { name: "恢复默认" })).toBeNull();
  fireEvent.change(select, { target: { value: "custom_3" } });
  fireEvent.change(screen.getByLabelText("润色提示词"), { target: { value: "第三个方案" } });
  fireEvent.change(select, { target: { value: "custom_2" } });
  fireEvent.change(select, { target: { value: "custom_3" } });
  expect((screen.getByLabelText("润色提示词") as HTMLTextAreaElement).value).toBe("第三个方案");
});

test("the AI prompt slot selector is no longer Linux-only", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
  // Windows users could author three custom prompts but had no control that
  // would ever select one, so prompt_id stayed at whatever it was.
  const slot = await screen.findByLabelText("AI 联想提示词方案");
  fireEvent.change(slot, { target: { value: "custom_2" } });
  expect((slot as HTMLSelectElement).value).toBe("custom_2");
  expect(screen.queryByText("兼容提示词")).toBeNull();
  expect(screen.queryByText(/Android 只发送选中文字/)).toBeNull();
});
