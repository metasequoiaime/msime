import type { Preferences } from "../core/preferences-model";
import { errorCode } from "../core/error-code";
import {
  cloudClipboardDeviceStatus,
  type CloudClipboardDeviceShape,
  type CloudClipboardDeviceStatusShape,
} from "../keyboard/cloud-response";
import type { CloudClipboardWebDavPreferences } from "./cloud-clipboard-relay-preferences";

/** 「我的词条」经 WebDAV 网盘同步的偏好 `dictionary_sync_webdav`：文件夹地址、网盘账号和应用密码，规则与云剪贴板的网盘相同，两者各配各的（`docs/dictionary-webdav-sync.md`）。 */
export type DictionarySyncWebDavPreferences = CloudClipboardWebDavPreferences;

/** `run` 的触发方式：改词之后（只记录、不联网）、打开设置、按 `next_check_ms` 定时、用户点「立即同步」。 */
export type DictionarySyncTrigger = "local_change" | "opened" | "periodic" | "manual";

/** `msime_client_dictionary_sync` 的 `action`，字段见 `crates/host-api/include/msime_client.h`。 */
export type DictionarySyncAction =
  | {
      operation: "run";
      trigger: DictionarySyncTrigger;
      confirm_deletions?: boolean;
      restore_deletions?: boolean;
    }
  | { operation: "status" }
  | { operation: "devices" }
  | { operation: "approve" | "reject" | "remove"; device_id: string }
  | { operation: "reset" }
  | { operation: "restart" }
  | { operation: "probe"; url: string; username: string; password: string; trust_remote?: boolean };

/** 宿主的词库同步：在工作线程上调 `msime_client_dictionary_sync`，`options`、`directory` 和设备名由宿主补上。成功时 resolve 成那个动作的 Value，失败时以带 `code` 的错误 reject（错误码见头文件）。没有它的宿主不显示「词库同步」。 */
export interface DictionarySyncClient {
  request(action: DictionarySyncAction): Promise<unknown>;
}

/** `status` 的结果，也是 `run` 结果里的 `status`。 */
export interface DictionarySyncStatus {
  /** 本机在词库密钥串里的设备编号和核对码；还没用过网盘时为 `null`。 */
  device_id: string | null;
  code: string | null;
  /** 本机持有当前密钥（已获准，或者是第一台设备）。 */
  approved: boolean;
  joined: boolean;
  words: number;
  pending_upload: number;
  pending_apply: number;
  /** 被挡下的大批删除：本机少了这么多词，等用户选择。 */
  held_deletions: number | null;
  clock_skew: boolean;
  last_success_ms: number | null;
  last_error: string | null;
  /** 自动同步因请求额度在等时什么时候继续（Unix 毫秒）：本机额度不够一轮，或者被网盘限流之后；没在等时为 `null`。「立即同步」不受它限制。 */
  budget_wait_until: number | null;
}

/** `run` 的结果：失败时没有 `report`，`error` 是错误码，`status` 照样给出。 */
export interface DictionarySyncRun {
  network: boolean;
  next_check_ms: number;
  status: DictionarySyncStatus;
  pendingKey: boolean;
  deferred: boolean;
  /** 这一次写进本机词库的改动数（网盘来的词和删除）；失败时为 0。 */
  applied: number;
  error: string | null;
}

function count(value: unknown): number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : 0;
}

function optionalText(value: unknown): string | null {
  return typeof value === "string" && value !== "" ? value : null;
}

/** 读出 `status`；形状不对时为 `null`。 */
export function dictionarySyncStatus(value: unknown): DictionarySyncStatus | null {
  if (!value || typeof value !== "object") return null;
  const status = value as Record<string, unknown>;
  if (typeof status.approved !== "boolean" || typeof status.joined !== "boolean") return null;
  const held = count(status.held_deletions);
  const success = count(status.last_success_ms);
  const budgetWait = count(status.budget_wait_until);
  return {
    device_id: optionalText(status.device_id),
    code: optionalText(status.code),
    approved: status.approved,
    joined: status.joined,
    words: count(status.words),
    pending_upload: count(status.pending_upload),
    pending_apply: count(status.pending_apply),
    held_deletions: held > 0 ? held : null,
    clock_skew: status.clock_skew === true,
    last_success_ms: success > 0 ? success : null,
    last_error: optionalText(status.last_error),
    budget_wait_until: budgetWait > 0 ? budgetWait : null,
  };
}

function twoDigits(value: number): string {
  return String(value).padStart(2, "0");
}

/** 自动同步在等额度时状态卡里的一句：本机时间的「HH:MM」，不是今天时写成「M月D日 HH:MM」；已经过了或者没在等时为 `null`。与 iOS、Android 和 Windows 的说法逐字相同。 */
export function dictionarySyncBudgetWaitMessage(
  until: number | null,
  now: number = Date.now(),
): string | null {
  if (until === null || until <= now) return null;
  const date = new Date(until);
  const today = new Date(now);
  if (Number.isNaN(date.getTime()) || Number.isNaN(today.getTime())) return null;
  const sameDay =
    date.getFullYear() === today.getFullYear() &&
    date.getMonth() === today.getMonth() &&
    date.getDate() === today.getDate();
  const time = `${twoDigits(date.getHours())}:${twoDigits(date.getMinutes())}`;
  const when = sameDay ? time : `${date.getMonth() + 1}月${date.getDate()}日 ${time}`;
  return `额度用完，将于 ${when} 继续自动同步；可点「立即同步」。`;
}

/** 读出 `run` 的结果（`remove`、`reset` 结果里的 `run` 也是这个形状）；形状不对时为 `null`。 */
export function dictionarySyncRun(value: unknown): DictionarySyncRun | null {
  if (!value || typeof value !== "object") return null;
  const run = value as Record<string, unknown>;
  const status = dictionarySyncStatus(run.status);
  if (!status) return null;
  const report =
    run.report && typeof run.report === "object" ? (run.report as Record<string, unknown>) : null;
  return {
    network: run.network === true,
    next_check_ms: count(run.next_check_ms),
    status,
    pendingKey: report?.pending_key === true,
    deferred: report?.deferred === true,
    applied: count(report?.applied),
    error: report ? null : (optionalText(run.error) ?? "dictionary_sync_unavailable"),
  };
}

/** 读出设备管理的结果（`devices`、`approve`、`reject`，以及 `remove`、`reset` 结果里的 `devices`），形状与云剪贴板的设备管理相同；不对时为 `null`。 */
export function dictionarySyncDevices(value: unknown): CloudClipboardDeviceStatusShape | null {
  if (!value || typeof value !== "object") return null;
  return cloudClipboardDeviceStatus(value as Parameters<typeof cloudClipboardDeviceStatus>[0]);
}

/** 设备列表按设备编号每台只留一条：同一台设备的几条登记合成一条，获准、本机任一条是就算是，名称取最后一条。返回已获准的（含本机）和等待允许的两组。 */
export function dictionarySyncDeviceGroups(devices: CloudClipboardDeviceShape[]): {
  approved: CloudClipboardDeviceShape[];
  waiting: CloudClipboardDeviceShape[];
} {
  const merged = new Map<string, CloudClipboardDeviceShape>();
  for (const device of devices) {
    const seen = merged.get(device.device_id);
    merged.set(
      device.device_id,
      seen
        ? {
            ...seen,
            name: device.name || seen.name,
            approved: seen.approved || device.approved,
            current: seen.current || device.current,
          }
        : device,
    );
  }
  const all = [...merged.values()];
  return {
    approved: all.filter((device) => device.approved || device.current),
    waiting: all.filter((device) => !device.approved && !device.current),
  };
}

/** 偏好里是否配置了词库同步。 */
export function dictionarySyncConfigured(preferences: Preferences | undefined): boolean {
  return Boolean(preferences?.dictionary_sync_webdav?.url);
}

/** 写入或清除词库同步的网盘配置。清除时去掉这个键而不是写空值，与 Rust 没配置时不写进文档的形状一致。与云剪贴板的网盘互不影响。 */
export function withDictionarySyncWebDav(
  preferences: Preferences,
  webdav: DictionarySyncWebDavPreferences | undefined,
): Preferences {
  if (webdav) return { ...preferences, dictionary_sync_webdav: webdav };
  const next = { ...preferences };
  delete next.dictionary_sync_webdav;
  return next;
}

/** 词库同步失败时给人看的一句话。`probe` 为真时是「测试连接」的结果：`dictionary_sync_unsupported` 在那里说的是网盘不合格，在同步时说的是网盘上的格式比本机新。错误码见 `crates/host-api/include/msime_client.h` 的 `msime_client_dictionary_sync`。 */
export function dictionarySyncErrorMessage(error: unknown, probe = false): string {
  const code = typeof error === "string" ? error : errorCode(error);
  switch (code) {
    case "dictionary_sync_not_configured":
      return "地址、账号或应用密码不完整。";
    case "invalid_dictionary_sync":
      return "请求无效，请关闭设置后重新打开再试。";
    case "dictionary_sync_unauthorized":
      return "网盘拒绝了这个账号或应用密码。";
    case "dictionary_sync_unavailable":
      return "连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。";
    case "dictionary_sync_rate_limited":
      return "网盘暂时限制了请求次数，或本机这 30 分钟的同步次数已用完，请稍后再试。";
    case "dictionary_sync_too_large":
      return "网盘上的词库或本机的同步记录超过了上限，删减一些词条后再试。";
    case "dictionary_sync_rollback":
      return "网盘上的词库比本机读到过的旧，或者被删改过。如果是你自己清理了网盘上的文件夹，请确认网盘现状并恢复同步。本机词条保留，但未上传的删除可能重新合并回来。";
    case "dictionary_sync_unsupported":
      return probe
        ? "这个网盘不支持词库同步需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹。"
        : "网盘上的词库是更新版本的格式，请先更新水杉输入法。";
    case "dictionary_sync_corrupt":
      return "网盘上的词库记录损坏了，自动同步已暂停。";
    case "dictionary_sync_storage":
      return "本机的同步记录读写失败。";
    case "clipboard_device_pending":
      return "本设备还在等待允许。";
    case "not_found":
      return "这台设备已经不在列表里了，请刷新后再试。";
    case "clipboard_key_conflict":
    case "clipboard_keystore_conflict":
      return "另一台设备同时修改了密钥，请稍后再试。";
    case "clipboard_device_limit":
      return "设备数已达上限，请先移除不再使用的设备。";
  }
  return probe ? "测试连接失败，请稍后再试。" : "同步失败，请稍后再试。";
}

/** 一次 `periodic` 最早和最晚在多久之后：Rust 给的 `next_check_ms` 不小于 1 秒，这里再给它一个上限，免得超过浏览器定时器能表示的范围。 */
export const DICTIONARY_SYNC_MIN_DELAY_MS = 1_000;
export const DICTIONARY_SYNC_MAX_DELAY_MS = 30 * 60 * 1000;

/** 下一次 `periodic` 在多少毫秒后。 */
export function dictionarySyncDelay(nextCheckMs: number): number {
  return Math.min(
    DICTIONARY_SYNC_MAX_DELAY_MS,
    Math.max(DICTIONARY_SYNC_MIN_DELAY_MS, Math.floor(nextCheckMs)),
  );
}
