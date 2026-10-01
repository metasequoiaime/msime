import type {
  AccountClient,
  AiSkinProposal,
  ChatClient,
  CommunityResourceApplication,
  CommunityResourcePage,
  CommunitySkin,
  CommunitySkinDownload,
  CommunitySkinPage,
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
  const baseAccount: AccountClient = {
    status: () => invoke("account_status"),
    providers: () => invoke("account_providers"),
    requestCode: (provider, target) => invoke("account_request_code", { provider, target }),
    login: (challengeId, code) => invoke("account_login", { challengeId, code }),
    profile: () => invoke("account_profile"),
    rename: (displayName) => invoke("account_rename", { displayName }),
    logout: (all) => invoke("account_logout", { all }),
    deleteAccount: () => invoke("account_delete"),
    clearExpired: () => invoke("account_forget"),
  };
  const accountChat: ChatClient = {
    models: () => invoke("account_chat_models"),
    complete: (messages, model) =>
      invoke<{ content: string }>("account_chat", { messages, model }).then(
        (response) => response.content,
      ),
  };
  const accountSettingsSync: SettingsSyncClient = {
    schema: () => invoke("account_preferences_schema"),
    load: () => invoke("account_preferences_load"),
    upload: () => invoke("account_preferences_upload"),
    apply: (userId, preferences) => invoke("account_preferences_apply", { userId, preferences }),
  };
  const account: AccountClient =
    platform === "ios"
      ? {
          ...baseAccount,
          appleLogin: () =>
            invoke<{ user?: { id: string; displayName: string; createdAt: string } | null }>(
              "account_apple_login",
            ),
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
    communitySkins: {
      list: (offset, search, mine, category) =>
        invoke<CommunitySkinPage>("community_skin_list", { offset, search, mine, category }),
      detail: (id) => invoke("community_skin_detail", { id }),
      download: (id, name) =>
        invoke<CommunitySkinDownload>("community_skin_download", { id, name }),
      rate: (id, stars) => invoke("community_skin_rate", { id, stars }),
      publish: (id, name, description, design, category) =>
        invoke("community_skin_publish", { id, name, description, design, category }),
      unpublish: (id) => invoke("community_skin_unpublish", { id }),
      setCategory: (id, category) =>
        invoke<CommunitySkin>("community_skin_set_category", { id, category }),
      finishTrial: (id, keep) => invoke("community_skin_finish_trial", { id, keep }),
      report: (id, reason, detail) =>
        invoke("community_report", { kind: "skins", id, reason, detail }),
    },
    aiSkins: {
      generate: (requestId, prompt) =>
        invoke<AiSkinProposal[]>("ai_skin_generate", { requestId, prompt }),
      cancel: (requestId) => invoke("ai_skin_cancel", { requestId }),
      onProgress: (listener) =>
        listen<{ requestId: string; completed: number }>("ai-skin-progress", (event) =>
          listener(event.payload),
        ),
    },
    communityResources: {
      list: (kind, scope, search, offset) =>
        invoke<CommunityResourcePage>("community_resource_list", { kind, scope, search, offset }),
      detail: (id) => invoke("community_resource_detail", { id }),
      publish: (id, kind, name, description, content, revision) =>
        invoke("community_resource_publish", { id, kind, name, description, content, revision }),
      apply: (id, resourceRevision) =>
        invoke<CommunityResourceApplication>("community_resource_apply", { id, resourceRevision }),
      save: (id, saved) => invoke("community_resource_save", { id, saved }),
      rate: (id, stars) => invoke("community_resource_rate", { id, stars }),
      unpublish: (id) => invoke("community_resource_unpublish", { id }),
      storeReply: (item) => invoke("community_resource_store_reply", { item }),
      removeReply: (id) => invoke("community_resource_remove_reply", { id }),
      report: (kind, id, reason, detail) =>
        invoke("community_report", {
          kind: kind === "dictionary" ? "dictionaries" : "replies",
          id,
          reason,
          detail,
        }),
    },
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
