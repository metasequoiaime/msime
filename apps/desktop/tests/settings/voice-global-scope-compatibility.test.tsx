// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";

afterEach(cleanup);

test.each([undefined, false, true])(
  "调整已有快捷键保留未展示的跨输入法范围：%s",
  async (global) => {
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
          asr_provider: "local",
          ...(global === undefined ? {} : { hotkey_global: global }),
        },
      },
    };
    const save = vi.fn(async (_revision, preferences) => ({
      ...snapshot,
      revision: 3,
      preferences,
    }));
    render(
      <SettingsPage
        client={{ load: async () => snapshot, save, host: testHost({ platform: "macos" }) }}
      />,
    );
    await settingsFormReady();
    fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
    const hold = await screen.findByRole("combobox", { name: "按住录音" });
    expect(screen.queryByRole("combobox", { name: "快捷键生效范围", hidden: true })).toBeNull();
    fireEvent.change(hold, { target: { value: "hotkey_rctrl_ralt" } });
    saveSettingsNow();
    await waitFor(() => expect(save).toHaveBeenCalled());
    const voice = save.mock.lastCall![1].voice_input;
    expect(voice.hotkey_global).toBe(global);
    expect(voice.hotkey_ralt).toBe(false);
    expect(voice.hotkey_rctrl_ralt).toBe(true);
  },
);
