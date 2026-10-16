// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ChatClient, ChatModels } from "../../../../packages/ui/src/chat/chat-model";
import { useChatConversation } from "../../../../packages/ui/src/chat/use-chat-conversation";
import { CHAT_SYSTEM_PROMPT } from "../../../../packages/ui/src/chat/chat-prompt";

afterEach(cleanup);

const catalog: ChatModels = {
  data: [{ id: "synthetic-default" }],
  defaultModel: "synthetic-default",
};

function client(overrides: Partial<ChatClient> = {}): ChatClient {
  return {
    models: async () => catalog,
    complete: async () => "合成回复",
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}

test("独立会话用默认模型和共享上下文发送，状态不在实例之间共享", async () => {
  const complete = vi.fn<ChatClient["complete"]>(async () => "合成回复");
  const host = client({ complete });
  const { result } = renderHook(() => ({
    first: useChatConversation({ client: host }),
    second: useChatConversation({ client: host }),
  }));
  await waitFor(() => expect(result.current.first.loadingModels).toBe(false));
  act(() => result.current.first.setDraft("  合成消息  "));
  await act(async () => {
    result.current.first.send();
    result.current.first.send();
  });
  expect(complete).toHaveBeenCalledExactlyOnceWith(
    [
      { role: "system", content: CHAT_SYSTEM_PROMPT },
      { role: "user", content: "合成消息" },
    ],
    catalog.defaultModel,
  );
  expect(result.current.first.messages.map(({ role, content }) => ({ role, content }))).toEqual([
    { role: "user", content: "合成消息" },
    { role: "assistant", content: "合成回复" },
  ]);
  expect(result.current.first.draft).toBe("");
  expect(result.current.second.messages).toEqual([]);
  expect(result.current.second.draft).toBe("");
});

test.each(["成功", "失败"])("旧客户端目录%s不能覆盖新目录或解除它的忙碌状态", async (outcome) => {
  const oldCatalog = deferred<ChatModels>();
  const nextCatalog = deferred<ChatModels>();
  const oldHost = client({ models: () => oldCatalog.promise });
  const models = vi.fn(() => nextCatalog.promise);
  const nextHost = client({ models });
  const { result, rerender } = renderHook(({ host }) => useChatConversation({ client: host }), {
    initialProps: { host: oldHost },
  });
  rerender({ host: nextHost });
  await act(async () => {
    if (outcome === "成功") oldCatalog.resolve(catalog);
    else oldCatalog.reject({ code: "account_unauthorized" });
  });
  expect(result.current.catalog).toBeNull();
  expect(result.current.loadingModels).toBe(true);
  expect(result.current.loginNeeded).toBe(false);
  expect(result.current.error).toBe("");
  act(() => void result.current.loadModels());
  expect(models).toHaveBeenCalledOnce();

  const current = { data: [{ id: "synthetic-next" }], defaultModel: "synthetic-next" };
  await act(async () => nextCatalog.resolve(current));
  expect(result.current.catalog).toEqual(current);
  expect(result.current.selectedModel).toBe(current.defaultModel);
  expect(result.current.loadingModels).toBe(false);
});

test("重新激活会话只刷新目录，保留消息和草稿并采用新默认模型", async () => {
  const nextCatalog = deferred<ChatModels>();
  const models = vi.fn<ChatClient["models"]>();
  models.mockResolvedValueOnce(catalog).mockReturnValueOnce(nextCatalog.promise);
  const complete = vi.fn<ChatClient["complete"]>(async () => "合成回复");
  const host = client({ models, complete });
  const { result, rerender } = renderHook(
    ({ active }) => useChatConversation({ client: host, active }),
    { initialProps: { active: true } },
  );
  await waitFor(() => expect(result.current.loadingModels).toBe(false));
  act(() => result.current.setDraft("第一轮"));
  await act(async () => result.current.send());
  const messages = result.current.messages;
  act(() => result.current.setDraft("第二轮"));
  rerender({ active: false });
  expect(models).toHaveBeenCalledOnce();
  rerender({ active: true });
  expect(models).toHaveBeenCalledTimes(2);
  expect(result.current.messages).toEqual(messages);
  expect(result.current.draft).toBe("第二轮");
  await act(async () =>
    nextCatalog.resolve({ data: [{ id: "synthetic-next" }], defaultModel: "synthetic-next" }),
  );
  await act(async () => result.current.send());
  expect(complete.mock.calls[1]).toEqual([
    [
      { role: "system", content: CHAT_SYSTEM_PROMPT },
      { role: "user", content: "第一轮" },
      { role: "assistant", content: "合成回复" },
      { role: "user", content: "第二轮" },
    ],
    "synthetic-next",
  ]);
});

test.each(
  ["取消", "更换客户端"].flatMap((operation) =>
    ["成功", "失败"].map((outcome) => ({ operation, outcome })),
  ),
)(
  "$operation 后旧回复$outcome 不能覆盖新请求或解除它的忙碌状态",
  async ({ operation, outcome }) => {
    const oldReply = deferred<string>();
    const nextReply = deferred<string>();
    const complete = vi.fn<ChatClient["complete"]>();
    complete.mockReturnValueOnce(oldReply.promise).mockReturnValueOnce(nextReply.promise);
    const oldHost = client({ complete });
    const nextHost = client({ complete: () => nextReply.promise });
    const { result, rerender } = renderHook(({ host }) => useChatConversation({ client: host }), {
      initialProps: { host: oldHost },
    });
    await waitFor(() => expect(result.current.loadingModels).toBe(false));
    act(() => result.current.setDraft("过期消息"));
    act(() => result.current.send());
    if (operation === "更换客户端") {
      rerender({ host: nextHost });
      await waitFor(() => expect(result.current.loadingModels).toBe(false));
    }
    // 保留现有交互：取消当前发送后，用户可继续发送。
    act(() => result.current.cancel());
    act(() => result.current.setDraft("当前消息"));
    act(() => result.current.send());
    await act(async () => {
      if (outcome === "成功") oldReply.resolve("过期回复");
      else oldReply.reject({ code: "account_unauthorized" });
    });
    expect(result.current.sending).toBe(true);
    expect(result.current.error).toBe("");
    expect(result.current.loginNeeded).toBe(false);
    expect(result.current.messages.some(({ content }) => content === "过期回复")).toBe(false);
    await act(async () => nextReply.resolve("当前回复"));
    expect(result.current.sending).toBe(false);
    expect(result.current.messages.at(-1)?.content).toBe("当前回复");
  },
);

test("清空使在途回复过期，并保留交给页面处理的草稿", async () => {
  const reply = deferred<string>();
  const host = client({ complete: () => reply.promise });
  const { result } = renderHook(() => useChatConversation({ client: host }));
  await waitFor(() => expect(result.current.loadingModels).toBe(false));
  act(() => result.current.setDraft("合成消息"));
  act(() => result.current.send());
  act(() => result.current.setDraft("保留草稿"));
  act(() => result.current.clear());
  await act(async () => reply.resolve("过期回复"));
  expect(result.current.messages).toEqual([]);
  expect(result.current.draft).toBe("保留草稿");
  expect(result.current.sending).toBe(false);
});
