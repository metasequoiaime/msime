import { useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { errorCode } from "../core/error-code";
import * as chat from "./chat-style";
import { boundedHistory, chatMessageByteLength, MAX_MESSAGE_BYTES } from "./chat-history";
import { chatError } from "./chat-errors";

export type ChatMessage = {
  role: "user" | "assistant" | "system";
  content: string;
};

export type ChatModel = { id: string };

export type ChatModels = {
  data: ChatModel[];
  defaultModel: string;
};

export interface ChatClient {
  models(): Promise<ChatModels>;
  complete(messages: ChatMessage[], model: string): Promise<string>;
}

type DisplayMessage = ChatMessage & { id: number };

// Mirrors the client-core byte and message limits used by the chat service.

export function ChatPage({
  client,
  onLogin,
  autoFocus = false,
  touch = false,
}: {
  client: ChatClient;
  onLogin?: () => void;
  autoFocus?: boolean;
  /** A host with no hardware keyboard: the chord still works if one is attached, but naming it is noise. */
  touch?: boolean;
}) {
  const [catalog, setCatalog] = useState<ChatModels | null>(null);
  const [selectedModel, setSelectedModel] = useState("");
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const { confirm, confirmation } = useConfirm();
  const [draft, setDraft] = useState("");
  const [loadingModels, setLoadingModels] = useState(true);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState("");
  const [loginNeeded, setLoginNeeded] = useState(false);
  const generation = useRef(0);
  const modelGeneration = useRef(0);
  const mounted = useRef(true);
  const sendingRef = useRef(false);
  const modelsRunning = useRef(false);
  const nextMessageId = useRef(1);
  const composer = useRef<HTMLTextAreaElement>(null);

  const loadModels = async () => {
    if (!mounted.current || modelsRunning.current) return;
    modelsRunning.current = true;
    const current = ++modelGeneration.current;
    setLoadingModels(true);
    setError("");
    setLoginNeeded(false);
    try {
      const value = await client.models();
      if (!mounted.current || modelGeneration.current !== current) return;
      setCatalog(value);
      setSelectedModel((current) =>
        value.data.some((model) => model.id === current) ? current : value.defaultModel,
      );
    } catch (cause) {
      if (!mounted.current || modelGeneration.current !== current) return;
      const unauthorized = errorCode(cause) === "account_unauthorized";
      setLoginNeeded(unauthorized);
      setError(chatError(cause));
    } finally {
      if (modelGeneration.current === current) modelsRunning.current = false;
      if (mounted.current && modelGeneration.current === current) setLoadingModels(false);
    }
  };

  useEffect(() => {
    mounted.current = true;
    void loadModels();
    return () => {
      mounted.current = false;
      modelGeneration.current += 1;
      generation.current += 1;
      sendingRef.current = false;
      modelsRunning.current = false;
    };
  }, [client]);

  useEffect(() => {
    if (!autoFocus) return;
    const timer = window.setTimeout(() => composer.current?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [autoFocus]);

  const requestReply = async (history: DisplayMessage[]) => {
    if (sending || sendingRef.current || !selectedModel) return;
    const version = ++generation.current;
    sendingRef.current = true;
    setSending(true);
    setError("");
    try {
      const reply = await client.complete(boundedHistory(history), selectedModel);
      if (generation.current !== version) return;
      setMessages((current) => [
        ...current,
        { id: nextMessageId.current++, role: "assistant", content: reply },
      ]);
    } catch (cause) {
      if (generation.current !== version) return;
      const unauthorized = errorCode(cause) === "account_unauthorized";
      setLoginNeeded(unauthorized);
      setError(chatError(cause));
    } finally {
      if (generation.current === version) {
        sendingRef.current = false;
        setSending(false);
      }
    }
  };

  const send = () => {
    const content = draft.trim();
    if (!content || sending) return;
    if (loginNeeded) {
      onLogin?.();
      return;
    }
    if (!selectedModel) return;
    // Refused before it joins the conversation; otherwise every retry would resend it and fail.
    if (chatMessageByteLength(content) > MAX_MESSAGE_BYTES) {
      setError("消息过长，请精简后再发送。");
      return;
    }
    const next = [...messages, { id: nextMessageId.current++, role: "user" as const, content }];
    setMessages(next);
    setDraft("");
    void requestReply(next);
  };

  const retry = () => {
    if (sending || !selectedModel || messages.at(-1)?.role !== "user") return;
    void requestReply(messages);
  };

  const cancel = () => {
    generation.current += 1;
    sendingRef.current = false;
    setSending(false);
  };

  const clear = () => {
    cancel();
    setMessages([]);
    setError("");
  };

  return (
    <section className={chat.page} aria-label="AI 对话">
      {confirmation}
      <div className={chat.toolbar}>
        <div>
          <strong>边聊天，边试键盘</strong>
          <small>使用共享账号与 EveryAPI 对话</small>
        </div>
        <div className={chat.modelControls}>
          {loadingModels ? (
            <span role="status">正在加载模型…</span>
          ) : (
            catalog && (
              <select
                aria-label="聊天模型"
                value={selectedModel}
                disabled={sending}
                onChange={(event) => setSelectedModel(event.target.value)}
              >
                {catalog.data.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.id}
                  </option>
                ))}
              </select>
            )
          )}
          <button
            type="button"
            className="secondary"
            disabled={loadingModels || sending}
            onClick={() => void loadModels()}
          >
            刷新模型
          </button>
        </div>
      </div>
      {loginNeeded && onLogin && (
        <button type="button" className={`primary ${chat.login}`} onClick={onLogin}>
          登录使用 AI
        </button>
      )}
      {error && (
        <div className={chat.error} role="alert">
          <span>{error}</span>
          {!loginNeeded && messages.at(-1)?.role === "user" && (
            <button type="button" className="secondary" disabled={sending} onClick={retry}>
              重试
            </button>
          )}
        </div>
      )}
      <div className={chat.messages} aria-live="polite">
        {messages.length === 0 && (
          <div className={chat.empty}>
            <strong>试试你的输入方案和键盘皮肤</strong>
            <span>
              发一条消息，看看水杉键盘在不同编辑器中的表现。对话内容只用于本次请求，不会写入输入统计。
            </span>
          </div>
        )}
        {messages.map((message) => (
          <div className={chat.message(message.role === "user")} key={message.id}>
            <span className={chat.bubble(message.role === "user")}>{message.content}</span>
          </div>
        ))}
        {sending && (
          <div className={chat.pending} role="status">
            正在回复…
          </div>
        )}
      </div>
      <div className={chat.composer}>
        <textarea
          ref={composer}
          aria-label="聊天消息"
          value={draft}
          maxLength={16_384}
          disabled={sending}
          placeholder="输入消息，试试键盘"
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              send();
            }
          }}
        />
        <div className={chat.actions}>
          <button
            type="button"
            className="secondary"
            disabled={!messages.length && !draft}
            onClick={() => {
              if (!messages.length) {
                clear();
                return;
              }
              void confirm({
                title: "开始新对话",
                message: "当前消息将被清空。",
                confirmLabel: "开始",
              }).then((confirmed) => {
                if (confirmed) clear();
              });
            }}
          >
            新对话
          </button>
          {sending ? (
            <button type="button" className="secondary" onClick={cancel}>
              取消
            </button>
          ) : (
            <button
              type="button"
              className="primary"
              disabled={!draft.trim() || (!loginNeeded && !selectedModel)}
              onClick={send}
            >
              发送
            </button>
          )}
        </div>
        <small>
          {touch ? "最多保留最近 14 条消息" : "Ctrl/⌘ + Enter 发送 · 最多保留最近 14 条消息"}
        </small>
      </div>
    </section>
  );
}
