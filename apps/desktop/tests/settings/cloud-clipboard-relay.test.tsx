// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  CLOUD_CLIPBOARD_RELAY_UNAUTHORIZED,
  CLOUD_CLIPBOARD_RELAY_UNSUPPORTED,
  CLOUD_CLIPBOARD_SIGNED_OUT,
  SettingsPage,
  cloudClipboardAvailabilityNote,
  cloudClipboardErrorMessage,
  cloudClipboardFailure,
  type Preferences,
} from "@msime/ui";
import {
  cloudClipboardRelayTokenIssue,
  cloudClipboardRelayUrlIssue,
  withCloudClipboardRelay,
} from "../../../../packages/ui/src/settings/cloud-clipboard-relay";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const TOKEN = "synthetic-relay-token-0001";

test("relay addresses follow the Rust rule: https, or http only to loopback, with no query, fragment or userinfo", () => {
  // 与 crates/client-services/src/clipboard_relay_tests.rs 的用例一一对应。
  for (const [url, valid] of [
    ["https://relay.example.test", true],
    ["https://relay.example.test:8443/msime/", true],
    ["http://127.0.0.1:8787", true],
    ["http://localhost/msime", true],
    ["http://[::1]:8787", true],
    ["http://relay.example.test", false],
    ["http://192.168.1.2:8787", false],
    ["https://relay.example.test/?token=x", false],
    ["https://relay.example.test?x", false],
    ["https://relay.example.test/?", false],
    ["https://relay.example.test/#x", false],
    ["https://relay.example.test/#", false],
    ["https://user@relay.example.test", false],
    ["https://user:secret@relay.example.test", false],
    ["https:///msime", false],
    ["wss://relay.example.test", false],
    ["relay.example.test", false],
    ["", false],
    ["https://relay.example.test/\u0007", false],
  ] as const) {
    expect(cloudClipboardRelayUrlIssue(url) === "", url).toBe(valid);
  }
  expect(cloudClipboardRelayUrlIssue(`https://relay.example.test/${"a".repeat(2048)}`)).toBe(
    "中继地址过长。",
  );
  expect(cloudClipboardRelayUrlIssue("http://relay.example.test")).toContain("HTTPS");
});

test("relay tokens are 16 to 256 printable ASCII characters", () => {
  expect(cloudClipboardRelayTokenIssue(TOKEN)).toBe("");
  expect(cloudClipboardRelayTokenIssue("a".repeat(16))).toBe("");
  expect(cloudClipboardRelayTokenIssue("a".repeat(256))).toBe("");
  for (const token of [
    "",
    "a".repeat(15),
    "a".repeat(257),
    "synthetic relay token",
    "synthetic-relay-令牌-0001",
  ]) {
    expect(cloudClipboardRelayTokenIssue(token), token).not.toBe("");
  }
});

test("clearing the relay removes the key instead of writing an empty value", () => {
  const base: Preferences = {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  };
  const configured = withCloudClipboardRelay(base, {
    url: "https://relay.example.test",
    token: TOKEN,
  });
  expect(configured.cloud_clipboard_relay).toEqual({
    url: "https://relay.example.test",
    token: TOKEN,
  });
  const cleared = withCloudClipboardRelay(configured, undefined);
  expect("cloud_clipboard_relay" in cleared).toBe(false);
  expect(cleared).toEqual(base);
});

test("a refused relay token is not read as signed out of the account", () => {
  const refused = { code: "relay_unauthorized" };
  expect(cloudClipboardFailure(refused)).toBe("relay_unauthorized");
  expect(cloudClipboardErrorMessage(refused, "失败")).toBe(CLOUD_CLIPBOARD_RELAY_UNAUTHORIZED);
  expect(cloudClipboardAvailabilityNote("relay_unauthorized")).toBe(
    CLOUD_CLIPBOARD_RELAY_UNAUTHORIZED,
  );
  expect(cloudClipboardFailure({ code: "account_unauthorized" })).toBe("signed_out");
  expect(cloudClipboardErrorMessage({ code: "account_unauthorized" }, "失败")).toBe(
    CLOUD_CLIPBOARD_SIGNED_OUT,
  );
  for (const code of [
    "relay_not_configured",
    "relay_unavailable",
    "relay_storage",
    "clipboard_full",
  ]) {
    expect(cloudClipboardFailure({ code })).toBe("unavailable");
    expect(cloudClipboardErrorMessage({ code }, "失败")).not.toBe("失败");
  }
});

test("a host that refuses the account while a relay is configured says so instead of asking to sign in", () => {
  // iOS、Android 的 Tauri 工程只能经水杉账号收发，配置了中继时 `cloud_clipboard_request` 答 relay_unsupported。
  const refused = { code: "relay_unsupported" };
  expect(cloudClipboardFailure(refused)).toBe("relay_unsupported");
  expect(cloudClipboardErrorMessage(refused, "失败")).toBe(CLOUD_CLIPBOARD_RELAY_UNSUPPORTED);
  expect(cloudClipboardAvailabilityNote("relay_unsupported")).toBe(
    CLOUD_CLIPBOARD_RELAY_UNSUPPORTED,
  );
});

function snapshot(preferences: Preferences, revision = 7) {
  return { format_version: 1, revision, preferences };
}

const base: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

test("the clipboard page saves a checked relay and switches back to the account", async () => {
  const save = vi.fn(async (revision: number, preferences: Preferences) =>
    snapshot(preferences, revision + 1),
  );
  render(
    <SettingsPage
      initialPage="tools"
      client={{
        load: vi.fn().mockResolvedValue(snapshot(base)),
        save,
        cloudClipboardRelay: true,
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("textbox", { name: "中继地址" })).toBeNull();
  fireEvent.click(await screen.findByRole("radio", { name: "自建中继" }));

  const saveButton = screen.getByRole("button", { name: "保存中继设置" }) as HTMLButtonElement;
  expect(saveButton.disabled).toBe(true);
  fireEvent.change(screen.getByRole("textbox", { name: "中继地址" }), {
    target: { value: "http://relay.example.test" },
  });
  expect(screen.getByText(/中继必须使用 HTTPS/)).toBeDefined();
  fireEvent.change(screen.getByRole("textbox", { name: "中继地址" }), {
    target: { value: "https://relay.example.test/msime" },
  });
  fireEvent.change(screen.getByLabelText("中继令牌"), { target: { value: "short" } });
  expect(saveButton.disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("中继令牌"), { target: { value: ` ${TOKEN} ` } });
  expect(saveButton.disabled).toBe(false);
  // 填写本身不写进设置，点了保存才写。
  saveSettingsNow();
  expect(save).not.toHaveBeenCalled();

  fireEvent.click(saveButton);
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(save.mock.calls[0][1].cloud_clipboard_relay).toEqual({
    url: "https://relay.example.test/msime",
    token: TOKEN,
  });
  expect(
    await screen.findByText(/云剪贴板现在经 https:\/\/relay\.example\.test\/msime 同步/),
  ).toBeDefined();

  fireEvent.click(screen.getByRole("radio", { name: "水杉账号" }));
  fireEvent.click(screen.getByRole("button", { name: "改回水杉账号" }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
  expect("cloud_clipboard_relay" in save.mock.calls[1][1]).toBe(false);
});

test("does not echo a saved relay token and keeps it when only the address changes", async () => {
  const save = vi.fn(async (revision: number, preferences: Preferences) =>
    snapshot(preferences, revision + 1),
  );
  render(
    <SettingsPage
      initialPage="tools"
      client={{
        load: vi.fn().mockResolvedValue(
          snapshot({
            ...base,
            cloud_clipboard_relay: { url: "https://relay.example.test/old", token: TOKEN },
          }),
        ),
        save,
        cloudClipboardRelay: true,
      }}
    />,
  );
  await settingsFormReady();

  const token = screen.getByLabelText("中继令牌") as HTMLInputElement;
  expect(token.value).toBe("");
  fireEvent.change(screen.getByRole("textbox", { name: "中继地址" }), {
    target: { value: "https://relay.example.test/new" },
  });
  const saveButton = screen.getByRole("button", { name: "保存中继设置" }) as HTMLButtonElement;
  expect(saveButton.disabled).toBe(false);
  fireEvent.click(saveButton);
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(save.mock.calls[0][1].cloud_clipboard_relay).toEqual({
    url: "https://relay.example.test/new",
    token: TOKEN,
  });
});

test("hosts that cannot route through a relay do not offer it", async () => {
  render(
    <SettingsPage
      initialPage="tools"
      client={{ load: vi.fn().mockResolvedValue(snapshot(base)), save: vi.fn() }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByText("云剪贴板同步方式")).toBeNull();
});

test("saving a relay on the clipboard page checks the cloud clipboard again over the new route", async () => {
  const save = vi.fn(async (revision: number, preferences: Preferences) =>
    snapshot(preferences, revision + 1),
  );
  // 没登录时账号路径答未登录；保存中继之后同一个请求经中继答可用。
  const cloudClipboardRequest = vi
    .fn()
    .mockRejectedValueOnce({ code: "account_unauthorized" })
    .mockResolvedValue({ enabled: true, items: [], e2e: { status: "ready" } });
  render(
    <SettingsPage
      initialPage="tools"
      client={{
        load: vi.fn().mockResolvedValue(snapshot({ ...base, clipboard_history: true })),
        save,
        cloudClipboardRelay: true,
        cloudClipboardRequest,
        clipboard: {
          clear: vi.fn(),
          list: vi
            .fn()
            .mockResolvedValue([
              { text: "合成剪贴板记录", timestampMs: 1_700_000_000_000, pinned: false },
            ]),
        },
      }}
    />,
  );
  await settingsFormReady();
  expect(await screen.findByText(CLOUD_CLIPBOARD_SIGNED_OUT)).toBeDefined();
  const send = await screen.findByRole("button", { name: "发到云剪贴板" });
  expect(send.hasAttribute("disabled")).toBe(true);

  fireEvent.click(screen.getByRole("radio", { name: "自建中继" }));
  fireEvent.change(screen.getByRole("textbox", { name: "中继地址" }), {
    target: { value: "https://relay.example.test/msime" },
  });
  fireEvent.change(screen.getByLabelText("中继令牌"), { target: { value: TOKEN } });
  fireEvent.click(screen.getByRole("button", { name: "保存中继设置" }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));

  await waitFor(() => expect(cloudClipboardRequest).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "发到云剪贴板" }).hasAttribute("disabled")).toBe(
      false,
    ),
  );
  expect(screen.queryByText(CLOUD_CLIPBOARD_SIGNED_OUT)).toBeNull();
});
