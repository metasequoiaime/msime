import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  ClipboardHistoryClient,
  CloudClipboardPanelClient,
  CloudDictionaryPanelClient,
  EmojiCatalogGroup,
  EmojiPanelClient,
  PanelClient,
  SettingsClient,
  VoicePanelClient,
} from "@msime/ui";
import { createVoiceRecognitionClient } from "../voice/voice-recognition-client";

type DesktopPanelClients = {
  keyboard: PanelClient;
  handwriting: PanelClient;
  voice: VoicePanelClient;
  cloudClipboard: CloudClipboardPanelClient;
  cloudDictionary: CloudDictionaryPanelClient;
  emoji: EmojiPanelClient;
};

/** 只装配客户端；调用方在模块级创建一次，避免渲染时重置面板任务。 */
export function createDesktopPanelClients(
  preferences: Pick<SettingsClient, "load" | "save">,
  cloudDictionary: CloudDictionaryPanelClient,
  clipboard: Required<ClipboardHistoryClient>,
): DesktopPanelClients {
  return {
    keyboard: {
      beginWindowDrag: () => getCurrentWindow().startDragging(),
      close: () => invoke("close_panel", { label: "keyboard-panel" }),
      openVoice: () => invoke("open_voice_panel"),
      rememberInputTarget: () => invoke("remember_input_target"),
      sendKey: (request) => invoke("send_key", { request }),
    },
    handwriting: {
      beginWindowDrag: () => getCurrentWindow().startDragging(),
      close: () => invoke("close_panel", { label: "handwriting-panel" }),
      rememberInputTarget: () => invoke("remember_input_target"),
      recognizeHandwriting: (request) => invoke("recognize_handwriting", { request }),
      submitHandwritingCandidate: (candidate) =>
        invoke("submit_handwriting_candidate", { candidate }),
      copyHandwritingCandidate: (text) => invoke("copy_text", { text }),
    },
    voice: {
      ...textPanelClient("voice-panel"),
      maxSubmitBytes: 4096,
      beginWindowDrag: () => getCurrentWindow().startDragging(),
      loadVoiceLanguage: () => invoke<string>("voice_input_language"),
      ...createVoiceRecognitionClient(invoke, (listener) =>
        listen<{
          request_id: string;
          text: string;
          final: boolean;
          phase?: "recording" | "recognizing" | "polishing";
          level?: number;
        }>("voice-update", (event) => listener(event.payload)),
      ),
      sendVoiceText: (text) => invoke("send_voice_text", { text }),
    },
    cloudClipboard: {
      ...textPanelClient("cloud-clipboard-panel"),
      canSendText: () => invoke<boolean>("cloud_clipboard_can_send_text"),
      request: (action) => invoke("cloud_clipboard_request", { action }),
    },
    cloudDictionary,
    emoji: {
      ...textPanelClient("emoji-panel"),
      beginWindowDrag: () => getCurrentWindow().startDragging(),
      resizeWindow: (edge) =>
        getCurrentWindow().startResizeDragging(
          (
            {
              n: "North",
              s: "South",
              e: "East",
              w: "West",
              ne: "NorthEast",
              nw: "NorthWest",
              se: "SouthEast",
              sw: "SouthWest",
            } as const
          )[edge],
        ),
      loadCatalog: () =>
        invoke<{
          emoji: EmojiCatalogGroup[];
          kaomoji: EmojiCatalogGroup[];
          symbols: EmojiCatalogGroup[];
          unavailable?: ("emoji" | "kaomoji" | "symbols")[];
        }>("load_emoji_catalog"),
      clipboard: {
        list: () => clipboard.list().then((entries) => entries.map((entry) => entry.text)),
        isEnabled: async () => (await preferences.load()).preferences.clipboard_history ?? false,
        enable: async () => {
          const snapshot = await preferences.load();
          if (!snapshot.preferences.clipboard_history) {
            await preferences.save(snapshot.revision, {
              ...snapshot.preferences,
              clipboard_history: true,
            });
          }
        },
        onChanged: async (listener) => {
          const stopHistory = await listen("clipboard-history-changed", () => listener());
          try {
            const stopPreferences = await listen("preferences-changed", () => listener());
            return () => {
              stopHistory();
              stopPreferences();
            };
          } catch (error) {
            stopHistory();
            throw error;
          }
        },
        remove: (text) => clipboard.remove(text),
        clear: () => clipboard.clear(),
        sync: () => clipboard.sync().then((entries) => entries.map((entry) => entry.text)),
        copy: (text) => clipboard.copy(text),
      },
    },
  };
}

function textPanelClient(label: string) {
  return {
    close: () => invoke<void>("close_panel", { label }),
    rememberInputTarget: () => invoke<void>("remember_input_target"),
    sendText: (text: string) => invoke<void>("send_text", { text }),
    copyText: (text: string) => invoke<void>("copy_text", { text }),
  };
}
