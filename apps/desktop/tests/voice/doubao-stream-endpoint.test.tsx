// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";

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

async function openVoice(
  voice: Record<string, unknown>,
  platform = "windows",
  save: SettingsClient["save"] = vi.fn(),
) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshotWith(voice),
        save,
        host: testHost({ platform }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
}

// 两个流式预设和自定义地址写的都是 `asr_endpoint`，合成一个控件：选预设就写它的地址，选「自定义地址」才出现输入框。
test("the two Doubao streaming interfaces are offered by name and write the address", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...snapshotWith({}),
    revision: 3,
    preferences,
  }));
  await openVoice({ asr_provider: "doubao", asr_endpoint: asyncEndpoint }, "windows", save);
  fireEvent.click(screen.getByRole("button", { name: "服务配置" }));

  const stream = screen.getByLabelText("流式接口") as HTMLSelectElement;
  expect([...stream.options].map((option) => option.value)).toEqual([
    "nostream",
    "async",
    "custom",
  ]);
  expect(stream.value).toBe("async");
  expect(screen.queryByLabelText("识别接口地址")).toBeNull();

  fireEvent.change(stream, { target: { value: "nostream" } });
  expect((screen.getByLabelText("流式接口") as HTMLSelectElement).value).toBe("nostream");
  expect(screen.queryByLabelText("识别接口地址")).toBeNull();
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].voice_input?.asr_endpoint).toBe(wholeSentenceEndpoint);

  // 选「自定义地址」时输入框带着当前地址出现，改它写的还是同一个值。
  fireEvent.change(screen.getByLabelText("流式接口"), { target: { value: "custom" } });
  const address = screen.getByLabelText("识别接口地址") as HTMLInputElement;
  expect(address.value).toBe(wholeSentenceEndpoint);
  fireEvent.change(address, { target: { value: "wss://synthetic.invalid/asr" } });
  expect((screen.getByLabelText("流式接口") as HTMLSelectElement).value).toBe("custom");
});

// 地址为空时豆包用默认的双向流式，下拉照实显示它，不显示成自定义。
test("an empty address reads as the default interface", async () => {
  await openVoice({ asr_provider: "doubao", asr_endpoint: "" });
  expect((screen.getByLabelText("流式接口") as HTMLSelectElement).value).toBe("async");
  expect(screen.queryByLabelText("识别接口地址")).toBeNull();
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
