// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CHAT_SYSTEM_PROMPT, ChatPage, type ChatClient, type ChatMessage } from "@msime/ui";
import { answerConfirm } from "../support/confirm";

afterEach(cleanup);

function client(overrides: Partial<ChatClient> = {}): ChatClient {
  return {
    models: async () => ({
      data: [{ id: "fixture-chat" }, { id: "fixture-fast" }],
      defaultModel: "fixture-chat",
    }),
    complete: async (_messages: ChatMessage[], _model: string) => "fixture reply",
    ...overrides,
  };
}

/** 等模型目录加载完：页面不再让用户选模型，只在内部用目录给的默认模型。 */
async function catalogLoaded() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

// 产品上不让用户选模型：页面没有模型下拉框和「刷新模型」，发送时固定用目录给的默认模型，它失败时由服务端换备用模型。
test("sends with the catalog's default model, offers no model choice, and starts a new conversation", async () => {
  const complete = vi.fn(async (messages: ChatMessage[], model: string) => {
    expect(messages).toEqual([
      { role: "system", content: CHAT_SYSTEM_PROMPT },
      { role: "user", content: "fixture prompt" },
    ]);
    expect(model).toBe("fixture-chat");
    return "fixture reply";
  });
  render(<ChatPage client={client({ complete })} />);

  await catalogLoaded();
  expect(screen.queryByRole("combobox", { name: "聊天模型" })).toBeNull();
  expect(screen.queryByRole("button", { name: "刷新模型" })).toBeNull();
  fireEvent.change(screen.getByRole("textbox", { name: "聊天消息" }), {
    target: { value: "fixture prompt" },
  });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await screen.findByText("fixture reply");
  expect(complete).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole("button", { name: "新对话" }));
  // The conversation survives until the question is answered.
  expect(screen.queryByText("fixture prompt")).toBeTruthy();
  await answerConfirm("confirm");
  expect(screen.queryByText("fixture prompt")).toBeNull();
  expect(screen.queryByText("fixture reply")).toBeNull();
});

test("a message over the host's byte bound is refused before it is sent", async () => {
  const complete = vi.fn(async () => "fixture reply");
  render(<ChatPage client={client({ complete })} />);
  await catalogLoaded();
  // 6,000 CJK characters are well under maxLength but 18,000 UTF-8 bytes.
  const long = "测".repeat(6_000);
  fireEvent.change(screen.getByRole("textbox", { name: "聊天消息" }), {
    target: { value: long },
  });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  expect((await screen.findByRole("alert")).textContent).toContain("消息过长，请精简后再发送。");
  expect(complete).not.toHaveBeenCalled();
  // The draft is kept so it can be shortened.
  expect((screen.getByRole("textbox", { name: "聊天消息" }) as HTMLTextAreaElement).value).toBe(
    long,
  );
});

test("sending ignores a same-tick duplicate submission", async () => {
  const pending = deferred<string>();
  const complete = vi.fn().mockReturnValue(pending.promise);
  render(<ChatPage client={client({ complete })} />);
  await catalogLoaded();
  fireEvent.change(screen.getByRole("textbox", { name: "聊天消息" }), {
    target: { value: "duplicate fixture" },
  });
  const send = screen.getByRole("button", { name: "发送" });
  act(() => {
    fireEvent.click(send);
    fireEvent.click(send);
  });
  expect(complete).toHaveBeenCalledOnce();
  await act(async () => {
    pending.resolve("fixture reply");
  });
  await screen.findByText("fixture reply");
});

// 模型目录没加载到（不是因为没登录）时，错误后面的「重试」重新加载目录，代替原来的「刷新模型」。
test("a failed model catalog is retried from the error", async () => {
  const models = vi
    .fn()
    .mockRejectedValueOnce({ code: "account_unavailable" })
    .mockResolvedValue({ data: [{ id: "fixture-chat" }], defaultModel: "fixture-chat" });
  const complete = vi.fn(async () => "fixture reply");
  render(<ChatPage client={client({ models, complete })} />);
  await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  await catalogLoaded();
  expect(models).toHaveBeenCalledTimes(2);
  fireEvent.change(screen.getByRole("textbox", { name: "聊天消息" }), {
    target: { value: "after retry" },
  });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await screen.findByText("fixture reply");
  expect(complete).toHaveBeenCalledOnce();
});

test("shows an actionable error and retries the latest user message", async () => {
  const complete = vi.fn<(messages: ChatMessage[], model: string) => Promise<string>>();
  complete
    .mockRejectedValueOnce({ code: "account_unavailable" })
    .mockResolvedValueOnce("retried reply");
  render(<ChatPage client={client({ complete })} />);
  await catalogLoaded();
  fireEvent.change(screen.getByRole("textbox", { name: "聊天消息" }), {
    target: { value: "retry fixture" },
  });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  await screen.findByText("retried reply");
  expect(complete).toHaveBeenCalledTimes(2);
});

test("does not render a late response after cancellation", async () => {
  let resolveReply: (value: string) => void = () => {};
  const complete = vi.fn(
    () =>
      new Promise<string>((resolve) => {
        resolveReply = resolve;
      }),
  );
  render(<ChatPage client={client({ complete })} />);
  await catalogLoaded();
  fireEvent.change(screen.getByRole("textbox", { name: "聊天消息" }), {
    target: { value: "cancel fixture" },
  });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  resolveReply("late fixture reply");
  await waitFor(() => expect(screen.queryByText("late fixture reply")).toBeNull());
  expect(screen.queryByText("正在回复…")).toBeNull();
});

test("ignores a late model catalog after the chat page unmounts", async () => {
  let resolveModels: (value: { data: { id: string }[]; defaultModel: string }) => void = () => {};
  const models = vi.fn(
    () =>
      new Promise<{ data: { id: string }[]; defaultModel: string }>((resolve) => {
        resolveModels = resolve;
      }),
  );
  const view = render(<ChatPage client={client({ models })} />);
  view.unmount();
  resolveModels({ data: [{ id: "late-model" }], defaultModel: "late-model" });
  await Promise.resolve();
});

test("prompts for login when the account backend rejects model loading", async () => {
  const onLogin = vi.fn();
  render(
    <ChatPage
      client={client({ models: vi.fn().mockRejectedValue({ code: "account_unauthorized" }) })}
      onLogin={onLogin}
    />,
  );
  expect(await screen.findByText("登录后即可与 AI 对话。")).toBeDefined();
  const composer = screen.getByRole("textbox", { name: "聊天消息" });
  expect((composer as HTMLTextAreaElement).disabled).toBe(false);
  fireEvent.change(composer, { target: { value: "未登录也能试键盘" } });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  expect(onLogin).toHaveBeenCalledOnce();
  expect((composer as HTMLTextAreaElement).value).toBe("未登录也能试键盘");
  fireEvent.click(screen.getByRole("button", { name: "登录使用 AI" }));
  expect(onLogin).toHaveBeenCalledTimes(2);
});

test("can autofocus the composer for the mobile keyboard tryout", async () => {
  render(<ChatPage client={client()} autoFocus />);
  const composer = await screen.findByRole("textbox", { name: "聊天消息" });
  await waitFor(() => expect(document.activeElement).toBe(composer));
});

// 触屏布局和「试用键盘」同一套样子：顶部只有「新对话」一个小胶囊，没有模型选择和「刷新模型」；这一页是用来试键盘的，空对话时只请用户自己打几个字，不给代写的开头；发出的消息是气泡，回复按 Markdown 画但不带链接。
test("the touch layout invites typing, sends with the round button and draws replies without links", async () => {
  const complete = vi.fn(async () => "**好的**，见 [这里](https://example.invalid)");
  render(<ChatPage client={client({ complete })} touch />);
  await catalogLoaded();
  expect(screen.queryByRole("button", { name: "刷新模型" })).toBeNull();
  expect(screen.queryByText(/最多保留最近/)).toBeNull();

  expect(screen.getByText("打几个字试试键盘")).toBeTruthy();
  const composer = screen.getByRole("textbox", { name: "聊天消息" });
  expect(composer.getAttribute("placeholder")).toBe("打几个字试试");
  fireEvent.change(composer, { target: { value: "水杉输入法" } });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));

  expect(await screen.findByText("水杉输入法", { selector: "p" })).toBeTruthy();
  const reply = await screen.findByText("好的");
  expect(reply.tagName).toBe("STRONG");
  expect(document.querySelector("a")).toBeNull();
  expect(screen.queryByText("打几个字试试键盘")).toBeNull();
});

// 触屏上没登录时不画红框报错，对话区正中是一句说明和「登录使用 AI」。
test("the touch layout asks to sign in in the middle of the conversation", async () => {
  const onLogin = vi.fn();
  render(
    <ChatPage
      client={client({ models: vi.fn().mockRejectedValue({ code: "account_unauthorized" }) })}
      touch
      onLogin={onLogin}
    />,
  );
  expect(await screen.findByText("登录后边聊边试键盘")).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "登录使用 AI" }));
  expect(onLogin).toHaveBeenCalledOnce();
});
