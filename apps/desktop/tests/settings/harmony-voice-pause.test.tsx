// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

// 两行的标题和说明与 iOS 原生语音页逐字相同（#10310）；说明段讲的是鸿蒙上其他识别服务怎么结束。
const pauseTitle = "本地停顿时间";
const sensitivityTitle = "判停灵敏度";
const sensitivityDescription =
  "数值越高越容易判定说完、结束录音；调得太高，轻声说的字可能被当成停顿";
const footer =
  "这两项只用于点按开始的本地识别，只保存在本机，下次录音生效；判停灵敏度只对带语音活动检测的本地整段模型生效。系统识别仍在停顿 1.5 秒后结束，云服务由「说完了」或松开结束。";

function snapshot(voice: Record<string, unknown> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 3,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      voice_input: { enabled: true, language: "zh-cn", asr_provider: "local", ...voice },
    },
  } as Snapshot;
}

function renderSettings({
  capability,
  mobile = true,
  voice,
}: {
  /** 宿主报告 `voice_pause_settings`；鸿蒙的设置桥只在手机和平板上报告。 */
  capability?: boolean;
  mobile?: boolean;
  voice?: Record<string, unknown>;
}) {
  const save = vi.fn().mockImplementation(async (_revision: number, next: unknown) => ({
    ...snapshot(),
    revision: 4,
    preferences: next,
  }));
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: vi.fn().mockResolvedValue(snapshot(voice)),
        save,
        host: testHost({
          platform: "harmony",
          mobile_settings: mobile,
          panel_windows: !mobile,
          ...(capability === undefined ? {} : { voice_pause_settings: capability }),
        }),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  return { save };
}

test.each([undefined, false])(
  "a Harmony phone that does not report voice_pause_settings (%s) lists neither slider",
  async (capability) => {
    renderSettings({ capability });
    await settingsFormReady();
    await screen.findByText("录音时静音其他声音");
    expect(screen.queryByRole("slider", { name: pauseTitle })).toBeNull();
    expect(screen.queryByRole("slider", { name: sensitivityTitle })).toBeNull();
    expect(screen.queryByText(footer)).toBeNull();
  },
);

test("the 2in1 voice page never lists them, even if a host reported the capability", async () => {
  renderSettings({ capability: true, mobile: false });
  await settingsFormReady();
  await screen.findAllByRole("tab");
  expect(screen.queryByRole("slider", { name: pauseTitle })).toBeNull();
  expect(screen.queryByRole("slider", { name: sensitivityTitle })).toBeNull();
});

test("a Harmony phone shows both sliders in 识别 with the old fixed values when the keys are missing", async () => {
  renderSettings({ capability: true });
  await settingsFormReady();
  const pause = (await screen.findByRole("slider", { name: pauseTitle })) as HTMLInputElement;
  const sensitivity = screen.getByRole("slider", { name: sensitivityTitle }) as HTMLInputElement;
  // 缺键按改成可调之前写死的值读：1500 毫秒，阈值 0.5 对应的 50。
  expect(pause.value).toBe("1500");
  expect([pause.min, pause.max, pause.step]).toEqual(["500", "5000", "100"]);
  expect(sensitivity.value).toBe("50");
  expect([sensitivity.min, sensitivity.max, sensitivity.step]).toEqual(["10", "90", "1"]);
  expect(screen.getByText(sensitivityDescription)).toBeTruthy();
  expect(screen.getByText(footer)).toBeTruthy();
});

test("each slider writes its own voice_input field and leaves the rest of voice_input alone", async () => {
  const { save } = renderSettings({ capability: true });
  await settingsFormReady();
  fireEvent.change(await screen.findByRole("slider", { name: pauseTitle }), {
    target: { value: "3000" },
  });
  fireEvent.change(screen.getByRole("slider", { name: sensitivityTitle }), {
    target: { value: "80" },
  });
  saveSettingsNow();
  await waitFor(() => expect(save.mock.calls.at(-1)?.[1].voice_input.stop_sensitivity).toBe(80));
  const voice = save.mock.calls.at(-1)?.[1].voice_input;
  expect(voice.pause_millis).toBe(3000);
  expect(voice.asr_provider).toBe("local");
});

test("stored values are shown as they are", async () => {
  renderSettings({ capability: true, voice: { pause_millis: 800, stop_sensitivity: 70 } });
  await settingsFormReady();
  expect(
    ((await screen.findByRole("slider", { name: pauseTitle })) as HTMLInputElement).value,
  ).toBe("800");
  expect((screen.getByRole("slider", { name: sensitivityTitle }) as HTMLInputElement).value).toBe(
    "70",
  );
});
