export type CloudClipboardItemShape = {
  id: string;
  text: string;
  pinned?: boolean;
  device?: string;
};
/** 云剪贴板端到端加密的本机状态：`ready` 可读写，`pending` 等待另一台设备允许，`disabled` 账号关闭了云剪贴板。 */
export type CloudClipboardE2eStatus = "ready" | "pending" | "disabled";
/** `list` 结果里的 `e2e` 对象，见 `docs/cloud-clipboard-e2e.md`。 */
export type CloudClipboardE2eShape = {
  status: CloudClipboardE2eStatus;
  /** 本机的核对码，16 位数字每 4 位一组。 */
  code: string;
  /** 等待本机允许的设备数。 */
  pending_devices: number;
  /** 本机解不开而被略去的条目数。 */
  undecryptable: number;
};
/** 账号下登记的一台云剪贴板设备。 */
export type CloudClipboardDeviceShape = {
  device_id: string;
  name: string;
  created_at: string;
  approved: boolean;
  /** 是否就是本机。 */
  current: boolean;
  code: string;
};
/** `devices`、`approve`、`reject`、`remove`、`reset` 的结果。 */
export type CloudClipboardDeviceStatusShape = {
  enabled: boolean;
  status: CloudClipboardE2eStatus;
  device_id: string;
  code: string;
  devices: CloudClipboardDeviceShape[];
};
export type CloudDictionaryEntryShape = {
  id: string;
  kind: string;
  code: string;
  word: string;
  weight: number;
  revision: number;
};
export type CloudDictionaryCatalogEntryShape = {
  kind: string;
  code: string;
  word: string;
  weight: number;
};
export type CloudCandidateShape = { code: string; canonical_pinyin?: string | null };

export function cloudResponseInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value);
}

export function cloudResponseText(value: {
  text?: unknown;
  content?: unknown;
}): string | undefined {
  return typeof value.text === "string"
    ? value.text
    : typeof value.content === "string"
      ? value.content
      : undefined;
}

export function cloudResponseRequest<T extends { status: string }>(value: {
  request?: unknown;
}): T | null {
  return value.request &&
    typeof value.request === "object" &&
    typeof (value.request as T).status === "string"
    ? (value.request as T)
    : null;
}

export function cloudClipboardItems(value: { items?: unknown }): CloudClipboardItemShape[] {
  return Array.isArray(value.items)
    ? value.items
        .filter(
          (item): item is CloudClipboardItemShape =>
            Boolean(item) &&
            typeof item === "object" &&
            typeof (item as CloudClipboardItemShape).id === "string" &&
            typeof (item as CloudClipboardItemShape).text === "string",
        )
        .map(({ id, text, pinned, device }) => ({
          id,
          text,
          // 元信息只在形状正确时保留：置顶必须是布尔值，来源设备必须是非空字符串。
          ...(typeof pinned === "boolean" ? { pinned } : {}),
          ...(typeof device === "string" && device.trim() ? { device: device.trim() } : {}),
        }))
    : [];
}

function cloudClipboardStatus(value: unknown): value is CloudClipboardE2eStatus {
  return value === "ready" || value === "pending" || value === "disabled";
}

/** 读出 `list` 结果里的 `e2e`；宿主没给或形状不对时为 `null`，面板按原来的方式只显示条目。 */
export function cloudClipboardE2e(value: { e2e?: unknown }): CloudClipboardE2eShape | null {
  const e2e = value.e2e as Partial<CloudClipboardE2eShape> | null | undefined;
  if (!e2e || typeof e2e !== "object" || !cloudClipboardStatus(e2e.status)) return null;
  const count = (field: unknown) => (cloudResponseInteger(field) && field >= 0 ? field : 0);
  return {
    status: e2e.status,
    code: typeof e2e.code === "string" ? e2e.code : "",
    pending_devices: count(e2e.pending_devices),
    undecryptable: count(e2e.undecryptable),
  };
}

/** 读出设备管理动作的结果；形状不对时为 `null`。 */
export function cloudClipboardDeviceStatus(value: {
  status?: unknown;
  devices?: unknown;
  device_id?: unknown;
  code?: unknown;
  enabled?: unknown;
}): CloudClipboardDeviceStatusShape | null {
  if (!cloudClipboardStatus(value.status) || !Array.isArray(value.devices)) return null;
  return {
    enabled: value.enabled !== false,
    status: value.status,
    device_id: typeof value.device_id === "string" ? value.device_id : "",
    code: typeof value.code === "string" ? value.code : "",
    devices: value.devices.filter(
      (device): device is CloudClipboardDeviceShape =>
        Boolean(device) &&
        typeof device === "object" &&
        typeof (device as CloudClipboardDeviceShape).device_id === "string" &&
        typeof (device as CloudClipboardDeviceShape).name === "string" &&
        typeof (device as CloudClipboardDeviceShape).created_at === "string" &&
        typeof (device as CloudClipboardDeviceShape).approved === "boolean" &&
        typeof (device as CloudClipboardDeviceShape).current === "boolean" &&
        typeof (device as CloudClipboardDeviceShape).code === "string",
    ),
  };
}

export function cloudDictionaryEntries<
  T extends CloudDictionaryEntryShape = CloudDictionaryEntryShape,
>(value: { entries?: unknown }): T[] {
  return Array.isArray(value.entries)
    ? value.entries.filter(
        (entry): entry is T =>
          Boolean(entry) &&
          typeof entry === "object" &&
          typeof (entry as T).id === "string" &&
          typeof (entry as T).kind === "string" &&
          typeof (entry as T).code === "string" &&
          typeof (entry as T).word === "string" &&
          cloudResponseInteger((entry as T).weight) &&
          cloudResponseInteger((entry as T).revision),
      )
    : [];
}

export function cloudDictionaryCatalogEntries<
  T extends CloudDictionaryCatalogEntryShape = CloudDictionaryCatalogEntryShape,
>(value: { catalog_entries?: unknown }): T[] {
  return Array.isArray(value.catalog_entries)
    ? value.catalog_entries.filter(
        (entry): entry is T =>
          Boolean(entry) &&
          typeof entry === "object" &&
          typeof (entry as T).kind === "string" &&
          typeof (entry as T).code === "string" &&
          typeof (entry as T).word === "string" &&
          cloudResponseInteger((entry as T).weight),
      )
    : [];
}

export function candidateMutationCode(candidate: CloudCandidateShape): string {
  return candidate.canonical_pinyin && candidate.canonical_pinyin.length > 0
    ? candidate.canonical_pinyin
    : candidate.code;
}
