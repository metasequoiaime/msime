import type { AccountClient } from "@msime/ui";

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/** 创建各 Tauri 宿主共用的基础账号客户端；平台能力由宿主组装层补充。 */
export function createAccountCommandClient(invoke: Invoke): AccountClient {
  return {
    namePresentation: (name, id = "") => invoke("account_name", { name, id }),
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
}
