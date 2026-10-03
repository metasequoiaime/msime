// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type HostCapabilities, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function snapshot(preferences: Partial<Snapshot["preferences"]> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      ...preferences,
    },
  };
}

async function openInputPage(
  host: HostCapabilities,
  preferences?: Partial<Snapshot["preferences"]>,
) {
  render(
    <SettingsPage client={{ load: async () => snapshot(preferences), save: vi.fn(), host }} />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  await screen.findByRole("radio", { name: "中文" });
}

function radio(name: string): HTMLInputElement {
  return screen.getByRole("radio", { name }) as HTMLInputElement;
}

test("a Windows capability shows 粤拼, 注音, 越南文 and 藏文 disabled", async () => {
  await openInputPage(
    testHost({
      platform: "windows",
      input_schemes: ["quanpin", "shuangpin", "wubi", "japanese", "korean"],
    }),
  );

  expect(radio("粤拼").disabled).toBe(true);
  expect(radio("注音").disabled).toBe(true);
  expect(radio("越南文").disabled).toBe(true);
  expect(radio("藏文").disabled).toBe(true);
  expect(radio("五笔").disabled).toBe(false);
  expect(screen.getByText("此平台暂不支持粤拼、注音")).toBeTruthy();
});

test("a macOS capability offers every scheme", async () => {
  await openInputPage(
    testHost({
      platform: "macos",
      input_schemes: [
        "quanpin",
        "shuangpin",
        "wubi",
        "japanese",
        "korean",
        "cantonese",
        "zhuyin",
        "vietnamese",
        "tibetan",
      ],
    }),
  );

  expect(radio("粤拼").disabled).toBe(false);
  expect(radio("注音").disabled).toBe(false);
  expect(radio("越南文").disabled).toBe(false);
  expect(screen.queryByText(/此平台暂不支持/)).toBeNull();
  fireEvent.click(radio("越南文"));
  expect(radio("Telex").checked).toBe(true);
  expect(radio("新式 hoà").checked).toBe(true);
  expect(radio("藏文").disabled).toBe(false);
  fireEvent.click(radio("藏文"));
  expect(radio("威利转写").checked).toBe(true);
  expect(screen.queryByRole("radio", { name: "Telex" })).toBeNull();
});

test("a Cantonese document on a host without Cantonese names the fallback", async () => {
  await openInputPage(testHost({ platform: "linux" }), {
    scheme: "cantonese",
    last_chinese_scheme: "cantonese",
  });

  // The selector's 粤拼, not the read-only 粤拼方案 row below it.
  const selector = radio("全拼").closest('[role="radiogroup"]') as HTMLElement;
  const cantonese = within(selector).getByRole("radio", { name: "粤拼" }) as HTMLInputElement;
  expect(cantonese.checked).toBe(true);
  expect(cantonese.disabled).toBe(true);
  expect(screen.getByText("此平台暂不支持粤拼、注音，已回退到全拼")).toBeTruthy();
});
