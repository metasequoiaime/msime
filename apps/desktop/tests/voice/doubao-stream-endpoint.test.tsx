// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const asyncEndpoint = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";
const wholeSentenceEndpoint = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream";

function snapshotWith(voice: Record<string, unknown>): Snapshot {
  return {
    format_version: 1,
    revision: 2,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      voice_input: { enabled: true, language: "zh-CN", ...voice },
    },
  } as Snapshot;
}

async function openVoice(voice: Record<string, unknown>, platform = "windows") {
  render(
    <SettingsPage
      client={{
        load: async () => snapshotWith(voice),
        save: vi.fn(),
        host: testHost({ platform }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
}

test("the two Doubao streaming interfaces are offered by name and write the address", async () => {
  await openVoice({ asr_provider: "doubao", asr_endpoint: asyncEndpoint });

  const stream = screen.getByLabelText("流式接口") as HTMLSelectElement;
  expect([...stream.options].map((option) => option.value)).toEqual([
    "nostream",
    "async",
    "custom",
  ]);
  expect(stream.value).toBe("async");

  fireEvent.change(stream, { target: { value: "nostream" } });
  expect((screen.getByLabelText("识别接口地址") as HTMLInputElement).value).toBe(
    wholeSentenceEndpoint,
  );
  expect((screen.getByLabelText("流式接口") as HTMLSelectElement).value).toBe("nostream");
});

test("an address that is neither preset reads as 自定义地址 and is not rewritten", async () => {
  await openVoice({ asr_provider: "doubao", asr_endpoint: "wss://synthetic.invalid/asr" });

  const stream = screen.getByLabelText("流式接口") as HTMLSelectElement;
  expect(stream.value).toBe("custom");

  // Selecting the placeholder names no endpoint, so it must leave the address alone.
  fireEvent.change(stream, { target: { value: "custom" } });
  expect((screen.getByLabelText("识别接口地址") as HTMLInputElement).value).toBe(
    "wss://synthetic.invalid/asr",
  );
});

test("the choice belongs to Doubao and is not shown for another provider", async () => {
  await openVoice({ asr_provider: "groq", asr_endpoint: "" });

  expect(screen.queryByLabelText("流式接口")).toBeNull();
  expect(screen.getByLabelText("识别接口地址")).toBeTruthy();
});
