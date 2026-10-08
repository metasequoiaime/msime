// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type {
  ImeSetupClient,
  ImeSetupState,
} from "../../../../packages/ui/src/core/host-contracts";
import type { ChatClient } from "../../../../packages/ui/src/chat/chat-page";
import { ToastProvider } from "../../../../packages/ui/src/core/toast";
import { TryKeyboardPage } from "../../../../packages/ui/src/keyboard/try-keyboard-page";

afterEach(cleanup);

function setup(state: ImeSetupState): ImeSetupClient {
  return { read: vi.fn(() => state), subscribe: vi.fn(() => () => undefined) };
}

function chatClient(overrides: Partial<ChatClient> = {}): ChatClient {
  return {
    models: vi.fn().mockResolvedValue({ data: [{ id: "model-a" }], defaultModel: "model-a" }),
    complete: vi.fn().mockResolvedValue("**你好**，我在。"),
    ...overrides,
  };
}

function renderPage(props: Parameters<typeof TryKeyboardPage>[0]) {
  render(
    <ToastProvider>
      <TryKeyboardPage {...props} />
    </ToastProvider>,
  );
  const page = screen.getByRole("region", { name: "试用键盘" });
  return { page, field: within(page).getByRole("textbox", { name: "试用键盘输入框" }) };
}

/** 对话区里的气泡文字，按出现顺序。 */
function bubbles(page: HTMLElement) {
  return [...within(page).getByRole("log", { name: "对话" }).children]
    .filter((element) => element.tagName !== "BUTTON")
    .map((element) => element.textContent?.trim());
}

function type(field: HTMLElement, text: string) {
  fireEvent.change(field, { target: { value: text } });
}

test("opens on the greeting with the field focused, so the keyboard comes up without a tap", () => {
  const { page, field } = renderPage({
    actions: { setup: setup({ enabled: true, current: true }) },
    chat: chatClient(),
  });

  expect(bubbles(page)).toEqual(["你好，我是水杉输入法。打几个字发给我试试，比如 shuishan。"]);
  expect(field.getAttribute("placeholder")).toBe("打字试试");
  expect(field.getAttribute("enterkeyhint")).toBe("send");
  expect(document.activeElement).toBe(field);
  expect(within(page).getByRole("button", { name: "发送" }).hasAttribute("disabled")).toBe(true);
  expect(within(page).getByRole("button", { name: "清空" }).hasAttribute("disabled")).toBe(true);
});

test("a host without AI chat offers the field alone, and Enter stays a line break", () => {
  const { page, field } = renderPage({
    actions: { setup: setup({ enabled: true, current: true }) },
  });

  expect(bubbles(page)).toEqual(["你好，我是水杉输入法。在下面打几个字试试，比如 shuishan。"]);
  expect(within(page).queryByRole("button", { name: "发送" })).toBeNull();
  expect(field.getAttribute("enterkeyhint")).toBe("enter");
  type(field, "shuishan");
  expect(fireEvent.keyDown(field, { key: "Enter" })).toBe(true);
});

test("Enter sends the draft as an AI chat and shows the reply as Markdown", async () => {
  const chat = chatClient();
  const { page, field } = renderPage({ chat });
  await waitFor(() => expect(chat.models).toHaveBeenCalledOnce());

  type(field, "  你好  ");
  fireEvent.keyDown(field, { key: "Enter" });

  expect((field as HTMLTextAreaElement).value).toBe("");
  await waitFor(() => expect(within(page).getByText("你好", { selector: "strong" })).toBeTruthy());
  expect(bubbles(page).slice(1)).toEqual(["你好", "你好，我在。"]);
  expect(chat.complete).toHaveBeenCalledWith(
    [
      { role: "system", content: expect.stringContaining("水杉输入法里的 AI 助手") },
      { role: "user", content: "你好" },
    ],
    "model-a",
  );
});

test("composition Enter and Shift+Enter do not send", async () => {
  const chat = chatClient();
  const { field } = renderPage({ chat });
  type(field, "ni");

  fireEvent.keyDown(field, { key: "Enter", isComposing: true });
  fireEvent.keyDown(field, { key: "Enter", shiftKey: true });

  await act(async () => {});
  expect(chat.complete).not.toHaveBeenCalled();
});

test("the conversation sent to the model leaves out the greeting and keeps the earlier turns", async () => {
  const chat = chatClient({
    complete: vi.fn().mockResolvedValueOnce("第一句").mockResolvedValueOnce("第二句"),
  });
  const { page, field } = renderPage({ chat });

  type(field, "一");
  fireEvent.click(within(page).getByRole("button", { name: "发送" }));
  await within(page).findByText("第一句");
  type(field, "二");
  fireEvent.click(within(page).getByRole("button", { name: "发送" }));
  await within(page).findByText("第二句");

  expect(vi.mocked(chat.complete).mock.calls[1][0].slice(1)).toEqual([
    { role: "user", content: "一" },
    { role: "assistant", content: "第一句" },
    { role: "user", content: "二" },
  ]);
});

test("a signed-out user is told why nothing came back and offered the sign-in", async () => {
  const onLogin = vi.fn();
  const chat = chatClient({
    models: vi.fn().mockRejectedValue({ code: "account_unauthorized" }),
  });
  const { page, field } = renderPage({ chat, onLogin });
  await waitFor(() => expect(chat.models).toHaveBeenCalledOnce());
  // 进页面时目录没拿到不打扰用户。
  expect(within(page).queryByRole("button", { name: "登录后使用 AI" })).toBeNull();

  type(field, "你好");
  fireEvent.keyDown(field, { key: "Enter" });

  expect(await within(page).findByText("登录后即可与 AI 对话。")).toBeTruthy();
  // 发送时重新试了一次目录。
  expect(chat.models).toHaveBeenCalledTimes(2);
  expect(chat.complete).not.toHaveBeenCalled();
  fireEvent.click(within(page).getByRole("button", { name: "登录后使用 AI" }));
  expect(onLogin).toHaveBeenCalledOnce();
});

test("停止 drops a reply that is still on its way", async () => {
  let answer: (text: string) => void = () => undefined;
  const chat = chatClient({
    complete: vi.fn(() => new Promise<string>((resolve) => (answer = resolve))),
  });
  const { page, field } = renderPage({ chat });

  type(field, "你好");
  fireEvent.keyDown(field, { key: "Enter" });
  const stop = await within(page).findByRole("button", { name: "停止" });
  expect(within(page).getByRole("status").textContent).toBe("正在回复…");

  fireEvent.click(stop);
  await act(async () => answer("太迟了"));

  expect(within(page).queryByText("太迟了")).toBeNull();
  expect(within(page).getByRole("button", { name: "发送" })).toBeTruthy();
  expect(bubbles(page).slice(1)).toEqual(["你好"]);
});

test("清空 goes back to the greeting and drops the draft", async () => {
  const chat = chatClient({ complete: vi.fn().mockResolvedValue("回复") });
  const { page, field } = renderPage({ chat });
  type(field, "你好");
  fireEvent.keyDown(field, { key: "Enter" });
  await within(page).findByText("回复");
  type(field, "草稿");

  fireEvent.click(within(page).getByRole("button", { name: "清空" }));

  expect(bubbles(page)).toEqual(["你好，我是水杉输入法。打几个字发给我试试，比如 shuishan。"]);
  expect((field as HTMLTextAreaElement).value).toBe("");
});

test("the draft is capped at 2000 characters, the same as Android and Apple", () => {
  const { field } = renderPage({ chat: chatClient() });
  expect((field as HTMLTextAreaElement).maxLength).toBe(2000);
});

test("the switch button names the step that is missing and takes it", () => {
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  const showInputMethodPicker = vi.fn().mockResolvedValue(undefined);
  const actions = { openSystemKeyboardSettings, showInputMethodPicker };

  renderPage({ actions: { ...actions, setup: setup({ enabled: false, current: false }) } });
  fireEvent.click(screen.getByRole("button", { name: "水杉还没启用？点此开启" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();

  cleanup();
  renderPage({ actions: { ...actions, setup: setup({ enabled: true, current: false }) } });
  fireEvent.click(screen.getByRole("button", { name: "当前不是水杉输入法？点此切换" }));
  expect(showInputMethodPicker).toHaveBeenCalledOnce();

  cleanup();
  renderPage({ actions: { ...actions, setup: setup({ enabled: true, current: true }) } });
  expect(screen.getByRole("button", { name: "切换输入法" })).toBeTruthy();
});

test("收起键盘 is offered while the field has focus and takes the focus away", () => {
  const { page, field } = renderPage({});
  const dismiss = within(page).getByRole("button", { name: "收起键盘" });

  fireEvent.click(dismiss);

  expect(document.activeElement).not.toBe(field);
  expect(within(page).queryByRole("button", { name: "收起键盘" })).toBeNull();
});

test("a link in a reply opens outside, and other schemes are ignored", async () => {
  const onOpenUrl = vi.fn();
  const chat = chatClient({
    complete: vi.fn().mockResolvedValue("[官网](https://msime.app) [坏的](javascript:alert(1))"),
  });
  const { page, field } = renderPage({ chat, onOpenUrl });
  type(field, "链接");
  fireEvent.keyDown(field, { key: "Enter" });

  fireEvent.click(await within(page).findByRole("link", { name: "官网" }));
  expect(onOpenUrl).toHaveBeenCalledWith("https://msime.app");
  // markdown-it 不把 javascript: 渲染成链接。
  expect(within(page).queryByRole("link", { name: "坏的" })).toBeNull();
});
