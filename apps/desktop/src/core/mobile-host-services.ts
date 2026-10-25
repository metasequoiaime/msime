import { createCommunityResourceClient } from "./community-resource-host";
import { createSettingsSyncClient } from "./settings-sync-host";
import { createAccountCommandClient } from "../account/account-command-client";
import { createCommunitySkinClient } from "@msime/ui";

import type {
  AccountClient,
  AccountUser,
  AiSkinProposal,
  ChatClient,
  CommunitySkinOperation,
  SettingsClient,
  SettingsSyncClient,
  MobileKeyboardFeedback,
} from "@msime/ui";

export type MobileHostPlatform = "android" | "ios";

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>;

export type MobileHostServiceDependencies = {
  invoke: Invoke;
  listen: Listen;
  navigateVoice: () => void;
};

/**
 * Services shared by the Android and iOS Tauri settings surfaces.
 *
 * The two hosts deliberately share account, community, AI and keyboard-skin
 * commands. Only the system keyboard-settings action, input-method picker and
 * native Apple sign-in differ, so those remain explicit platform branches.
 */
export function createMobileHostServices(
  platform: MobileHostPlatform,
  dependencies: MobileHostServiceDependencies,
): Partial<SettingsClient> {
  const { invoke, listen } = dependencies;
  const appIcon = {
    info: () => invoke<{ supported: boolean; selected: string }>("app_icon_info"),
    set: (style: string) =>
      invoke<{ supported: boolean; selected: string }>("app_icon_set", { style }),
  };
  const baseAccount = createAccountCommandClient(invoke);
  const accountChat: ChatClient = {
    models: () => invoke("account_chat_models"),
    complete: (messages, model) =>
      invoke<{ content: string }>("account_chat", { messages, model }).then(
        (response) => response.content,
      ),
  };
  const accountSettingsSync: SettingsSyncClient = createSettingsSyncClient(invoke);
  const communitySkinRequest = <T>(
    operation: CommunitySkinOperation,
    args: Record<string, unknown>,
  ): Promise<T> => {
    if (operation === "report") return invoke<T>("community_report", { kind: "skins", ...args });
    const command: Record<Exclude<CommunitySkinOperation, "report">, string> = {
      list: "community_skin_list",
      detail: "community_skin_detail",
      download: "community_skin_download",
      rate: "community_skin_rate",
      publish: "community_skin_publish",
      unpublish: "community_skin_unpublish",
      set_category: "community_skin_set_category",
      finish_trial: "community_skin_finish_trial",
    };
    return invoke<T>(command[operation], args);
  };
  const account: AccountClient =
    platform === "ios"
      ? {
          ...baseAccount,
          appleLogin: () => invoke<{ user?: AccountUser | null }>("account_apple_login"),
          settingsSync: accountSettingsSync,
        }
      : { ...baseAccount, settingsSync: accountSettingsSync };
  const openSystemKeyboardSettings =
    platform === "ios"
      ? () => invoke("open_system_keyboard_settings").then(() => undefined)
      : () => invoke("android_open_input_method_settings").then(() => undefined);

  const common: Partial<SettingsClient> = {
    appIcon,
    account,
    chat: accountChat,
    aiAssistant: {
      fetchModels: ({ endpoint, token }) => invoke<string[]>("ai_models", { endpoint, token }),
      test: ({ endpoint, model, prompt, token, text }) =>
        invoke<string>("ai_test", { endpoint, model, prompt, token, text }),
    },
    touchKeyboardSchemes: true,
    customTouchKeyboardSkins: true,
    customSkinLibrary: {
      load: () => invoke("load_custom_skin_library"),
      mutate: (action) => invoke("mutate_custom_skin_library", { action }),
    },
    communitySkins: createCommunitySkinClient(communitySkinRequest),
    aiSkins: {
      generate: (requestId, prompt) =>
        invoke<AiSkinProposal[]>("ai_skin_generate", { requestId, prompt }),
      cancel: (requestId) => invoke("ai_skin_cancel", { requestId }),
      onProgress: (listener) =>
        listen<{ requestId: string; completed: number }>("ai-skin-progress", (event) =>
          listener(event.payload),
        ),
    },
    communityResources: createCommunityResourceClient(invoke, { includeReplyActions: true }),
    openSystemKeyboardSettings,
    ...(platform === "android"
      ? { openScreenKeyboard: () => invoke("android_open_keyboard_tryout").then(() => undefined) }
      : {}),
    // iOS uses the shared Tauri settings pages for its keyboard preview and
    // quick entries; only the system-settings action crosses into native UI.
    ...(platform === "ios" ? { home: { openSystemKeyboardSettings } } : {}),
    ...(platform === "ios" ? { openVoice: async () => dependencies.navigateVoice() } : {}),
    mobileKeyboardFeedback: {
      load: () => invoke<MobileKeyboardFeedback>("mobile_keyboard_feedback_load"),
      save: (settings) =>
        invoke<MobileKeyboardFeedback>("mobile_keyboard_feedback_save", { settings }),
      preview: (strength) => invoke("mobile_keyboard_feedback_preview", { strength }),
    },
  };

  if (platform === "ios") {
    return common;
  }
  return {
    ...common,
    home: {
      openKeyboard: () => invoke("open_keyboard_panel"),
      openSystemKeyboardSettings: () => invoke("android_open_input_method_settings"),
      showInputMethodPicker: () => invoke("android_show_input_method_picker"),
    },
  };
}
