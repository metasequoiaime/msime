import { SendStopButton } from "../core/send-stop-button";
import MarkdownIt from "markdown-it";
import { useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { errorCode } from "../core/error-code";
import { errorMessage } from "../core/error-message";
import { useToast } from "../core/toast";
import { ActionButton } from "../core/action-button";
import * as chat from "./chat-style";
import { chatMessageByteLength, MAX_MESSAGE_BYTES } from "./chat-history";
import { resizeChatComposer } from "./chat-composer-size";
import { chatRequest } from "./chat-prompt";
import { chatError } from "./chat-errors";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useTimeout } from "../core/use-timeout";
import { useMountedRef } from "../settings/use-mounted-ref";
import {
  makeDefaultAction,
  useImeSetupState,
  type SetupActions,
} from "../keyboard/setup-status-card";

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

// 触屏上 AI 的回复按 Markdown 画，和「试用键盘」一样；关掉 HTML、图片和链接：设置页跑在 WebView 里，点链接会把整个设置页导航走。
const markdown = new MarkdownIt({ html: false, linkify: false, breaks: true }).disable([
  "image",
  "link",
]);
// Mirrors the client-core byte and message limits used by the chat service.

export function ChatPage({
  client,
  onLogin,
  autoFocus = false,
  touch = false,
  active = true,
  onComposerFocusChange,
  focusRequest = 0,
  setup,
}: {
  client: ChatClient;
  onLogin?: () => void;
  autoFocus?: boolean;
  /** 触屏上对话是常驻的底部标签，切走时页面只是隐藏。每次重新显示都重读模型列表，在「我的」里登录或退出后回来，看到的就是新账号的状态；对话记录保留。 */
  active?: boolean;
  /** 触屏输入框获得或失去焦点：外壳在打字时把底部标签栏让开，键盘上方只留输入栏。 */
  onComposerFocusChange?: (focused: boolean) => void;
  /** 每加一次就把焦点交给输入框，系统随之弹出键盘：首页的「试用键盘」切到这一页时用它。 */
  focusRequest?: number;
  /** 触屏上的设置引导：水杉没启用或不是当前输入法时，顶栏给一个去补完设置的胶囊，否则打出来的不是水杉。 */
  setup?: SetupActions;
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
  const generation = useAsyncGeneration(client);
  const modelGeneration = useAsyncGeneration(client);
  const mounted = useMountedRef();
  const sendingRef = useRef(false);
  const modelsRunning = useRef(false);
  const nextMessageId = useRef(1);
  const composer = useRef<HTMLTextAreaElement>(null);
  const [composerFocused, setComposerFocused] = useState(false);
  const setupState = useImeSetupState(setup?.setup);
  const toast = useToast();

  const loadModels = async () => {
    if (!mounted.current || modelsRunning.current) return;
    modelsRunning.current = true;
    const current = modelGeneration.current;
    setLoadingModels(true);
    setError("");
    setLoginNeeded(false);
    try {
      const value = await client.models();
      if (!mounted.current || modelGeneration.current !== current) return;
      setCatalog(value);
      // 产品上不让用户选模型：固定用服务端的默认模型，它失败时由服务端换备用模型重试。
      setSelectedModel(value.defaultModel);
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
    void loadModels();
    return () => {
      sendingRef.current = false;
      modelsRunning.current = false;
    };
  }, [client]);

  useEffect(() => {
    if (active) void loadModels();
  }, [active]);

  useEffect(() => {
    if (focusRequest) composer.current?.focus();
  }, [focusRequest]);

  useTimeout(() => composer.current?.focus(), autoFocus ? 0 : null);

  const requestReply = async (history: DisplayMessage[]) => {
    if (sending || sendingRef.current || !selectedModel) return;
    const version = ++generation.current;
    sendingRef.current = true;
    setSending(true);
    setError("");
    try {
      const reply = await client.complete(chatRequest(history), selectedModel);
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

  const resizeField = () => {
    resizeChatComposer(composer.current);
  };
  const newConversation = () => {
    if (!messages.length) {
      clear();
      setDraft("");
      return;
    }
    void confirm({ title: "开始新对话", message: "当前消息将被清空。", confirmLabel: "开始" }).then(
      (confirmed) => {
        if (confirmed) clear();
      },
    );
  };

  const setupAction =
    setupState?.enabled === false || setupState?.current === false
      ? makeDefaultAction(setup, setupState)
      : undefined;
  const runSetup = (action: () => Promise<void>) =>
    void action().catch((cause: unknown) => toast(errorMessage(cause)));

  // 触屏：这一页是用来试键盘的（打字发给 AI，看候选、皮肤和手感），所以不给代写的开头，让用户自己打。顶部一行胶囊（设置没补完时的引导、打字时的「收起键盘」、「新对话」），中间是气泡对话，底部圆角输入框加圆形发送键，和「试用键盘」同一套样子。没有登录、出错都写在对话里，不用红框。
  if (touch)
    return (
      <section className={chat.touchPage} aria-label="AI 对话">
        {confirmation}
        <div className={chat.touchBar}>
          {setupAction && (
            <button type="button" className={chat.touchChip} onClick={() => runSetup(setupAction)}>
              {setupState?.enabled === false
                ? "水杉还没启用？点此开启"
                : "当前不是水杉输入法？点此切换"}
            </button>
          )}
          {composerFocused && (
            <button
              type="button"
              className={chat.touchChip}
              // 按下时不让按钮抢走焦点，否则输入框先失焦、这个按钮随之消失，点按落空。
              onPointerDown={(event) => event.preventDefault()}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => composer.current?.blur()}
            >
              收起键盘
            </button>
          )}
          <button
            type="button"
            className={`${chat.touchChip} ml-auto`}
            disabled={!messages.length && !draft}
            onClick={newConversation}
          >
            新对话
          </button>
        </div>
        <div className={chat.touchLog} aria-live="polite">
          {loginNeeded && onLogin && messages.length === 0 ? (
            <div className={chat.touchEmpty}>
              <span className={chat.touchEmptyIcon} aria-hidden="true">
                <ChatGlyph />
              </span>
              <strong>登录后边聊边试键盘</strong>
              <span>用水杉账号登录，打几个字发给 AI，看看候选、皮肤和手感。</span>
              <button type="button" className={chat.touchLogin} onClick={onLogin}>
                登录使用 AI
              </button>
            </div>
          ) : (
            messages.length === 0 &&
            !error && (
              <div className={chat.touchEmpty}>
                <span className={chat.touchEmptyIcon} aria-hidden="true">
                  <ChatGlyph />
                </span>
                <strong>打几个字试试键盘</strong>
                <span>发给 AI，看看候选、皮肤和手感。对话只用于这次请求，不计入输入统计。</span>
              </div>
            )
          )}
          {messages.map((message) =>
            message.role === "user" ? (
              <p className={chat.touchMine} key={message.id}>
                {message.content}
              </p>
            ) : (
              <div
                className={chat.touchReply}
                key={message.id}
                // markdown-it 关闭了 HTML、链接和图片，这里只有它自己产生的段落、列表和代码。
                dangerouslySetInnerHTML={{ __html: markdown.render(message.content) }}
              />
            ),
          )}
          {sending && (
            <p className={chat.touchPending} role="status">
              正在回复…
            </p>
          )}
          {error && !(loginNeeded && messages.length === 0) && (
            <div className={chat.touchNotice} role="alert">
              <span>{error}</span>
              {loginNeeded && onLogin ? (
                <button type="button" className={chat.touchChip} onClick={onLogin}>
                  登录使用 AI
                </button>
              ) : (
                (!catalog || messages.at(-1)?.role === "user") && (
                  <button
                    type="button"
                    className={chat.touchChip}
                    disabled={sending || loadingModels}
                    // 还没拿到默认模型时重试的是加载，否则重发最后一条。
                    onClick={() => (catalog ? retry() : void loadModels())}
                  >
                    重试
                  </button>
                )
              )}
            </div>
          )}
        </div>
        <div className={chat.touchComposer}>
          <textarea
            ref={composer}
            rows={1}
            aria-label="聊天消息"
            className={chat.touchField}
            value={draft}
            maxLength={16_384}
            placeholder="打几个字试试"
            enterKeyHint="send"
            onFocus={() => {
              setComposerFocused(true);
              onComposerFocusChange?.(true);
            }}
            onBlur={() => {
              setComposerFocused(false);
              onComposerFocusChange?.(false);
            }}
            onChange={(event) => {
              setDraft(event.target.value);
              resizeField();
            }}
          />
          <SendStopButton
            className={chat.touchSend}
            sending={sending}
            sendDisabled={!draft.trim() || (!loginNeeded && !selectedModel)}
            onStop={cancel}
            onSend={() => {
              send();
              window.setTimeout(resizeField, 0);
            }}
          />
        </div>
      </section>
    );

  return (
    <section className={chat.page} aria-label="AI 对话">
      {confirmation}
      <div className={chat.toolbar}>
        <div>
          <strong>边聊天，边试键盘</strong>
          <small>使用共享账号与 EveryAPI 对话</small>
        </div>
      </div>
      {loginNeeded && onLogin && (
        <ActionButton action={onLogin} className={`primary ${chat.login}`} label="登录使用 AI" />
      )}
      {error && (
        <div className={chat.error} role="alert">
          <span>{error}</span>
          {!loginNeeded && (!catalog || messages.at(-1)?.role === "user") && (
            <ActionButton
              // 还没拿到默认模型时重试的是加载，否则重发最后一条。
              action={() => (catalog ? retry() : void loadModels())}
              className="secondary"
              disabled={sending || loadingModels}
              label="重试"
            />
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
          <ActionButton
            action={() => {
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
            className="secondary"
            disabled={!messages.length && !draft}
            label="新对话"
          />
          {sending ? (
            <ActionButton action={cancel} className="secondary" label="取消" />
          ) : (
            <ActionButton
              action={send}
              className="primary"
              disabled={!draft.trim() || (!loginNeeded && !selectedModel)}
              label="发送"
            />
          )}
        </div>
        <small>
          {touch ? "最多保留最近 14 条消息" : "Ctrl/⌘ + Enter 发送 · 最多保留最近 14 条消息"}
        </small>
      </div>
    </section>
  );
}

/** 空状态里的对话气泡图标。 */
function ChatGlyph() {
  return (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M4 5.5A2.5 2.5 0 0 1 6.5 3h11A2.5 2.5 0 0 1 20 5.5v8a2.5 2.5 0 0 1-2.5 2.5H10l-4.2 3.6a.5.5 0 0 1-.8-.4V16h0a2.5 2.5 0 0 1-1-2V5.5Z"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinejoin="round"
      />
      <path d="M8 8.5h8M8 11.5h5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}
