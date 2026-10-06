// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(cleanup);
test.each([
  [
    "tencent",
    "测试腾讯云翻译配置",
    "translation.tencent",
    { secret_id: "synthetic-id", secret_key: "synthetic-key", region: "ap-guangzhou" },
  ],
  [
    "niutrans",
    "测试 NiuTrans 配置",
    "translation.niutrans",
    { app_id: "synthetic-app", apikey: "synthetic-key" },
  ],
  [
    "custom",
    "测试自定义翻译配置",
    "translation.custom",
    { endpoint: "https://fixture.invalid/translate", api_key: "synthetic-key" },
  ],
] as const)(
  "Windows %s translation probe sends current settings only on click",
  async (kind, label, service, expected) => {
    const snapshot: Snapshot = {
      format_version: 1,
      revision: 1,
      preferences: {
        scheme: "quanpin",
        shuangpin_profile: "xiaohe",
        candidate_page_size: 5,
        learning: true,
        chinese_punctuation: true,
        candidate_translations: true,
        tencent_tmt: {
          enabled: kind === "tencent",
          secret_id: "synthetic-id",
          secret_key: "synthetic-key",
          region: "ap-guangzhou",
        },
        niutrans: {
          enabled: kind === "niutrans",
          app_id: "synthetic-app",
          apikey: "synthetic-key",
        },
        custom_translation: {
          enabled: kind === "custom",
          endpoint: "https://fixture.invalid/translate",
          api_key: "synthetic-key",
        },
      },
    };
    const probe = vi.fn().mockResolvedValue({ ok: true, message: "fixture complete" });
    render(
      <SettingsPage
        initialPage="expression"
        client={{
          load: async () => snapshot,
          save: vi.fn(),
          testApiCredential: probe,
          host: testHost({ platform: "windows" }),
        }}
      />,
    );
    const button = await screen.findByRole("button", { name: label });
    expect(probe).not.toHaveBeenCalled();
    fireEvent.click(button);
    await screen.findByText("fixture complete");
    expect(probe).toHaveBeenCalledWith(service, expected);
  },
);
