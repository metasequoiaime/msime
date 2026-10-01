// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(cleanup);
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
    niutrans: { enabled: true, app_id: "synthetic-app", apikey: "synthetic-key" },
  },
};

// 翻译相关的控件在「标点与翻译」页。macOS 菜单仍然请求 `input`，因为 `client-core` 目前能路由它；等路由器接受 `expression` 之后，这个入口应该落到的就是 `expression` 页。
test("macOS translation settings open on the 标点与翻译 page and save NiuTrans drafts", async () => {
  const save = vi.fn().mockResolvedValue(snapshot);
  const probe = vi.fn().mockResolvedValue({ ok: true, message: "synthetic success" });
  render(
    <SettingsPage
      initialPage="expression"
      client={{
        load: async () => snapshot,
        save,
        testApiCredential: probe,
        host: { platform: "macos" } as never,
      }}
    />,
  );
  const appId = await screen.findByLabelText("NiuTrans App ID");
  expect(screen.getByRole("heading", { name: "标点与翻译" })).toBeDefined();
  expect(probe).not.toHaveBeenCalled();
  fireEvent.change(appId, { target: { value: "synthetic-edited" } });
  fireEvent.click(screen.getByRole("button", { name: "测试 NiuTrans 配置" }));
  await screen.findByText("synthetic success");
  expect(probe).toHaveBeenCalledWith("translation.niutrans", {
    app_id: "synthetic-edited",
    apikey: "synthetic-key",
  });
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls[0][1].niutrans).toEqual({
    enabled: true,
    app_id: "synthetic-edited",
    apikey: "synthetic-key",
  });
});

test("macOS AI entry opens the shared AI category without implicit credential requests", async () => {
  const probe = vi.fn();
  render(
    <SettingsPage
      initialPage="ai"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        testApiCredential: probe,
        host: { platform: "macos" } as never,
      }}
    />,
  );
  expect(await screen.findByRole("heading", { name: "AI 辅助" })).toBeDefined();
  expect(await settingsFormReady()).toBeDefined();
  expect(probe).not.toHaveBeenCalled();
});

test("a menu entry picked while the page is open navigates without dropping the draft", async () => {
  const client = {
    load: async () => snapshot,
    save: vi.fn(),
    testApiCredential: vi.fn(),
    host: { platform: "macos" } as never,
  };
  const view = render(<SettingsPage initialPage="input" client={client} />);
  fireEvent.change(await screen.findByLabelText("NiuTrans App ID"), {
    target: { value: "synthetic-edited" },
  });

  view.rerender(<SettingsPage initialPage="ai" route={{ page: "ai", nonce: 1 }} client={client} />);
  expect(await screen.findByRole("heading", { name: "AI 辅助" })).toBeDefined();
  // The same section asked for again still counts as a request.
  view.rerender(
    <SettingsPage initialPage="input" route={{ page: "input", nonce: 2 }} client={client} />,
  );
  const appId = (await screen.findByLabelText("NiuTrans App ID")) as HTMLInputElement;
  expect(appId.value).toBe("synthetic-edited");
});
