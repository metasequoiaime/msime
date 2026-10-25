import { isComposingKeyEvent } from "../core/ime-key-event";
import { SendStopButton } from "../core/send-stop-button";
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import type { ChatClient, ChatMessage } from "../chat/chat-model";
import { chatRequest } from "../chat/chat-prompt";
import { resizeChatComposer } from "../chat/chat-composer-size";
import { chatError } from "../chat/chat-errors";
import { errorCode } from "../core/error-code";
import { touchHairline } from "../core/touch-style";
import { makeDefaultAction, useImeSetupState, type SetupActions } from "./setup-status-card";
import { useHostAction } from "../core/use-host-action";
import { createMarkdownLinkHandler, linkedMarkdownHtml } from "../core/linked-markdown";

/** 草稿上限，与 Android 和 Apple 的试用页相同。2000 个字符最多 8000 字节，在 client-services 单条消息 16 KiB 的上限之内，所以发送前不必再按字节检查。 */
const DRAFT_LIMIT = 2000;
/** 对话区最多保留的气泡数，超过时从最早的开始丢，与 Android 相同。 */
const BUBBLE_LIMIT = 60;
const GREETING = "你好，我是水杉输入法。打几个字发给我试试，比如 shuishan。";
const GREETING_WITHOUT_CHAT = "你好，我是水杉输入法。在下面打几个字试试，比如 shuishan。";
const openableLink = /^(https?:|mailto:)/i;

type Bubble = {
  id: number;
  /** `notice` 是水杉自己说的话（问候、失败说明），不进入发给模型的上下文。 */
  role: "user" | "assistant" | "notice";
  content: string;
};

// 细线用内阴影而不是 border：HarmonyOS 的 WebView 有缩放，1px 边框算出来是 0.92px，ArkWeb 重新光栅化这种圆角边框时会把按钮的下半截整块涂成边框色。

const chip = `m-0 shrink-0 cursor-pointer whitespace-nowrap rounded-full ${touchHairline} bg-[var(--p-group-bg)] px-3 py-1.5 text-[13px] [font-family:inherit] [color:var(--p-text)] active:opacity-70`;
const bubbleBase =
  "max-w-[80%] rounded-[18px] px-3.5 py-2.5 text-[15px] leading-[1.5] wrap-anywhere";
const sendButton =
  "m-0 flex size-10 shrink-0 cursor-pointer items-center justify-center rounded-full border-0 p-0 [background:var(--accent-color)] [color:var(--p-on-accent)] active:opacity-80 disabled:cursor-default disabled:opacity-40";

/**
 * 「首页」根页的子页面「试用键盘」，供自身没有键盘窗口可打开的宿主使用，按 Android 试用页的聊天样式：对话气泡（第一条是水杉的问候）、底部的输入框和圆形发送键，输入框上方一排切换输入法、收起键盘和清空。标题和返回由外壳绘制。
 *
 * 这里的键盘是系统绑定到输入框上的真正输入法。打的字不会被读取或保存；宿主提供 AI 对话时，点发送才把这一条作为 AI 对话发出去，回复显示在对话里。没有 AI 对话的宿主不显示发送键，页面只用来打字。
 */
export function TryKeyboardPage({
  actions,
  chat,
  onLogin,
  onOpenUrl,
  switchAway = true,
}: {
  actions?: SetupActions;
  chat?: ChatClient;
  onLogin?: () => void;
  onOpenUrl?: (url: string) => void;
  /** 水杉已启用且是当前输入法时，是否还提供「切换输入法」去换别的输入法。HarmonyOS 手机不提供：它打开的是系统的选择输入法弹窗，样子粗糙，键盘下方本来就有系统的切换键。 */
  switchAway?: boolean;
}) {
  const state = useImeSetupState(actions?.setup);
  const greeting = (): Bubble => ({
    id: 0,
    role: "notice",
    content: chat ? GREETING : GREETING_WITHOUT_CHAT,
  });
  const [bubbles, setBubbles] = useState<Bubble[]>(() => [greeting()]);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [focused, setFocused] = useState(false);
  const [loginNeeded, setLoginNeeded] = useState(false);
  const nextId = useRef(1);
  const field = useRef<HTMLTextAreaElement>(null);
  const log = useRef<HTMLDivElement>(null);
  // 模型目录：undefined 表示还在加载，null 表示没拿到。
  const model = useRef<string | null | undefined>(undefined);
  const catalog = useRef<Promise<void> | null>(null);
  // 上一次加载目录失败的原因，发送时用它说明为什么发不出去（典型是还没登录）。
  const catalogError = useRef<unknown>(null);
  // 每次发送、停止、清空或离开页面都加一，过期请求的结果被丢掉。
  const generation = useRef(0);

  useEffect(
    () => () => {
      generation.current += 1;
    },
    [],
  );

  const append = (role: Bubble["role"], content: string) =>
    setBubbles((current) => [
      ...current.slice(Math.max(0, current.length - BUBBLE_LIMIT + 1)),
      { id: nextId.current++, role, content },
    ]);

  const loadModels = () => {
    if (!chat) return Promise.resolve();
    catalog.current ??= chat.models().then(
      (value) => {
        model.current = value.defaultModel || value.data[0]?.id || null;
        catalogError.current = null;
      },
      (cause: unknown) => {
        // 没拿到目录不打扰用户：发送时再试一次，那时仍失败才在对话里说明原因。
        model.current = null;
        catalog.current = null;
        catalogError.current = cause;
      },
    );
    return catalog.current;
  };

  // 进页面就在后台加载模型目录，与 Android 一样不要用户先点「加载 AI」。
  useEffect(() => {
    void loadModels();
    // 目录只按进页面时的客户端加载一次。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chat]);

  // 打开这个页面就是要键盘，所以不等点按就把焦点交给输入框。
  useEffect(() => {
    field.current?.focus();
  }, []);

  // 新气泡出现时滚到底，只滚动对话区，不动焦点，键盘不会因此收起。
  useLayoutEffect(() => {
    const element = log.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [bubbles, sending]);

  const resizeField = () => {
    resizeChatComposer(field.current);
  };
  useLayoutEffect(resizeField, [draft]);

  const send = async () => {
    if (!chat || sending) return;
    const text = draft.trim();
    if (!text) return;
    const conversation: ChatMessage[] = [
      ...bubbles.flatMap((bubble): ChatMessage[] =>
        bubble.role === "notice" ? [] : [{ role: bubble.role, content: bubble.content }],
      ),
      { role: "user", content: text },
    ];
    append("user", text);
    setDraft("");
    const version = ++generation.current;
    setSending(true);
    setLoginNeeded(false);
    try {
      // 目录还没到时发出的这句已经画出来了，按钮是「停止」，目录到了再真正发出去。
      if (model.current === undefined || model.current === null) await loadModels();
      if (generation.current !== version) return;
      const selected = model.current;
      if (!selected) throw catalogError.current ?? new Error("no chat model");
      const reply = await chat.complete(chatRequest(conversation), selected);
      if (generation.current !== version) return;
      append("assistant", reply);
    } catch (cause) {
      if (generation.current !== version) return;
      const unauthorized = errorCode(cause) === "account_unauthorized";
      setLoginNeeded(unauthorized);
      append(
        "notice",
        errorCode(cause) ? chatError(cause) : "请求失败，请检查登录状态或稍后重试。",
      );
    } finally {
      if (generation.current === version) setSending(false);
    }
  };

  const stop = () => {
    generation.current += 1;
    setSending(false);
  };

  const clear = () => {
    generation.current += 1;
    setSending(false);
    setDraft("");
    setLoginNeeded(false);
    setBubbles([greeting()]);
  };

  const fieldKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // 聊天页的回车是发送；组字中的回车属于输入法，Shift+回车照常换行。
    if (event.key !== "Enter" || event.shiftKey || isComposingKeyEvent(event)) return;
    if (!chat) return;
    event.preventDefault();
    void send();
  };

  const openLink = createMarkdownLinkHandler(onOpenUrl, openableLink);

  const notReady = state?.enabled === false || state?.current === false;
  // 没启用或不是当前输入法时，这个按钮带用户把设置补完；都好了以后它只剩换走水杉这一个用处。
  const switchAction = notReady
    ? (makeDefaultAction(actions, state) ?? actions?.showInputMethodPicker)
    : switchAway
      ? actions?.showInputMethodPicker
      : undefined;
  const runHostAction = useHostAction();

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-2.5" aria-label="试用键盘">
      <div
        ref={log}
        className="-mx-1 flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto px-1 pt-1 pb-2"
        role="log"
        aria-label="对话"
        onClick={openLink}
        onAuxClick={openLink}
      >
        {bubbles.map((bubble) =>
          bubble.role === "user" ? (
            <p
              key={bubble.id}
              className={`${bubbleBase} m-0 self-end whitespace-pre-wrap [background:var(--accent-color)] [color:var(--p-on-accent)]`}
            >
              {bubble.content}
            </p>
          ) : bubble.role === "assistant" ? (
            <div
              key={bubble.id}
              className={`${bubbleBase} self-start bg-[var(--p-group-bg)] [color:var(--p-text)] [&_a]:underline [&_ol]:my-1 [&_ol]:pl-5 [&_p]:my-0 [&_p+p]:mt-2 [&_pre]:overflow-x-auto [&_pre]:whitespace-pre [&_ul]:my-1 [&_ul]:pl-5`}
              // markdown-it 关闭了 HTML，这里只有 Markdown 自己产生的元素。
              dangerouslySetInnerHTML={{ __html: linkedMarkdownHtml(bubble.content) }}
            />
          ) : (
            <p
              key={bubble.id}
              className={`${bubbleBase} m-0 self-start whitespace-pre-wrap bg-[var(--p-group-bg)] [color:var(--p-text)]`}
            >
              {bubble.content}
            </p>
          ),
        )}
        {sending && (
          <p
            className={`${bubbleBase} m-0 self-start bg-[var(--p-group-bg)] [color:var(--p-sub)]`}
            role="status"
          >
            正在回复…
          </p>
        )}
        {loginNeeded && onLogin && (
          <button
            type="button"
            className={`${chip} self-start [color:var(--p-accent-text,var(--accent-color))]`}
            onClick={onLogin}
          >
            登录后使用 AI
          </button>
        )}
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        {switchAction && (
          <button type="button" className={chip} onClick={() => runHostAction(switchAction)}>
            {state?.enabled === false
              ? "水杉还没启用？点此开启"
              : state?.current === false
                ? "当前不是水杉输入法？点此切换"
                : "切换输入法"}
          </button>
        )}
        {focused && (
          <button
            type="button"
            className={chip}
            // 按下时不让按钮抢走焦点，否则输入框先失焦、这个按钮随之消失，点按落空。
            onPointerDown={(event) => event.preventDefault()}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => field.current?.blur()}
          >
            收起键盘
          </button>
        )}
        <button
          type="button"
          className={`${chip} ml-auto`}
          disabled={bubbles.length === 1 && !draft && !sending}
          onPointerDown={(event) => event.preventDefault()}
          onMouseDown={(event) => event.preventDefault()}
          onClick={clear}
        >
          清空
        </button>
      </div>
      {notReady && !switchAction && (
        <p className="m-0 shrink-0 px-1 text-[13px] [color:var(--p-sub)]">
          先完成设置，键盘才会是水杉
        </p>
      )}
      <div className="flex shrink-0 items-end gap-2 pb-[env(safe-area-inset-bottom,0px)]">
        <textarea
          ref={field}
          rows={1}
          aria-label="试用键盘输入框"
          placeholder="打字试试"
          value={draft}
          maxLength={DRAFT_LIMIT}
          enterKeyHint={chat ? "send" : "enter"}
          className={`m-0 min-h-10 min-w-0 flex-1 resize-none rounded-[20px] ${touchHairline} bg-[var(--p-group-bg)] px-3.5 py-2 text-[16px] leading-[22px] [color:var(--p-text)] [font-family:inherit] outline-none placeholder:[color:var(--p-sub)]`}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={fieldKeyDown}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
        />
        {chat && (
          <SendStopButton
            className={sendButton}
            sending={sending}
            sendDisabled={!draft.trim()}
            onStop={stop}
            onSend={() => void send()}
          />
        )}
      </div>
    </section>
  );
}
