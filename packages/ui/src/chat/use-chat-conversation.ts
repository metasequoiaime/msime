import { useEffect, useRef, useState } from "react";
import type { ChatClient, ChatMessage, ChatModels } from "./chat-model";
import { chatMessageByteLength, MAX_MESSAGE_BYTES } from "./chat-history";
import { chatRequest } from "./chat-prompt";
import { chatError } from "./chat-errors";
import { errorCode } from "../core/error-code";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useMountedRef } from "../settings/use-mounted-ref";

type DisplayMessage = ChatMessage & { id: number };

/** 页面布局共享的对话状态与请求生命周期；焦点、确认框和输入框尺寸仍由页面持有。 */
export function useChatConversation({
  client,
  onLogin,
  active = true,
}: {
  client: ChatClient;
  onLogin?: () => void;
  active?: boolean;
}) {
  const [catalog, setCatalog] = useState<ChatModels | null>(null);
  const [selectedModel, setSelectedModel] = useState("");
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
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
    // 在加入对话前拒绝超限消息，否则每次重试都会带上它并再次失败。
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

  return {
    catalog,
    selectedModel,
    messages,
    draft,
    setDraft,
    loadingModels,
    sending,
    error,
    loginNeeded,
    loadModels,
    send,
    retry,
    cancel,
    clear,
  };
}
