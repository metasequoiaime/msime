/**
 * 设置应用和键盘经共享沙箱交换哪些状态、什么时候发布、什么时候导入。
 *
 * 两个进程各自保留私有的 `files/state`，原生层照旧从它推出偏好、`user/`、`cache/`。共享沙箱（`context.getGroupDir`）只用来交换对方要看到的那部分：发布方把文件复制到 `<共享目录>/<发布方>/`，最后写一份清单；导入方读清单，把指纹与上次导入不同的条目复制回自己的私有目录。不让两边直接共用一个状态目录，是因为 client-core 的存储连读都要在目录里建锁文件加锁，而基础访问模式下键盘对共享目录只读，直接共用时键盘连偏好都读不了；`prepare_host` 每次也要往状态目录里写。
 *
 * 指纹只看内容：偏好按 `preferences` 本身（不含修订号），小文件按全文，目录按每个文件的路径、大小和修改时间。导入写回的内容与对方相同，指纹也相同，所以不会再被发布回去，两边不会来回打转。
 */

import { PRIVACY_CONSENT_ENTRY, PRIVACY_CONSENT_FILE } from "../privacy/PrivacyConsentPolicy";
import {
  SCREEN_CAPTURE_PROTECTION_ENTRY,
  SCREEN_CAPTURE_PROTECTION_FILE,
} from "./settings/ScreenCaptureProtectionPolicy";

export type MirrorPublisher = "settings" | "keyboard";

/** `preferences` 经 `savePreferences` 写入，走存储的锁和修订号；`file` 是单个小文件；`directory` 是整棵目录。 */
export type MirrorKind = "preferences" | "file" | "directory";

export interface MirrorEntry {
  readonly name: string;
  readonly kind: MirrorKind;
  /** `state` 是相对 `files/state`，`files` 是相对 `files`。 */
  readonly base: "state" | "files";
  readonly path: string;
  /**
   * 只在键盘处于完整体验模式时交换。账号会话属于这一类：服务端会轮换刷新令牌，同一个刷新令牌用第二次会被拒绝，所以两边持有同一份会话时必须都能把刷新后的会话发布出去、由另一边的 Rust 采用；基础访问模式下键盘写不了共享目录，就不导入，继续用自己的匿名账号。
   */
  readonly fullModeOnly: boolean;
}

export interface ManifestEntry {
  name: string;
  /** 内容指纹；文件不存在时是 `ABSENT_FINGERPRINT`，导入方据此删除自己的副本。 */
  fingerprint: string;
  /** 发布方察觉到这条内容变化的时刻（毫秒）。两边都改过同一条时，较晚的一方为准。 */
  changedMs: number;
}

export interface ImportedEntry {
  name: string;
  fingerprint: string;
}

export interface MirrorManifest {
  format: number;
  publisher: MirrorPublisher;
  entries: ManifestEntry[];
  /** 键盘发布时附上它从设置应用导入到了哪一版，设置应用据此判断键盘的统计是否已经按最新的统计开关记录。 */
  applied: ImportedEntry[];
}

export interface ImportRecord {
  format: number;
  entries: ImportedEntry[];
}

export interface FingerprintedEntry {
  name: string;
  fingerprint: string;
}

export interface PublishPlan {
  manifest: MirrorManifest;
  /** 内容变了、要重新复制到共享目录的条目名。 */
  changed: string[];
}

export interface ListedFile {
  readonly path: string;
  readonly size: number;
  readonly mtime: number;
}

export interface StatisticsSwitch {
  enabled: boolean;
  retention: string;
  /** 设置应用里「清空统计」按过的次数。键盘看到它变大就清空自己的计数，否则导入的统计会把清掉的数字带回来。 */
  resets: number;
}

/** 导入一个普通文件之前调用方的决定：`copy` 照常复制；`handled` 调用方已经自己写好，不再复制；`retry` 这次不导入，下次再试。 */
export type ImportDecision = "copy" | "handled" | "retry";

export const MIRROR_FORMAT: number = 1;
export const ABSENT_FINGERPRINT: string = "absent";
export const MANIFEST_FILE: string = "manifest.json";
/** 设置应用写给键盘的统计开关，见 `StatisticsSwitch`。 */
export const STATISTICS_SWITCH_FILE: string = "typing-statistics-switch.json";
const RETENTIONS: string[] = ["forever", "30d", "90d", "180d", "365d"];

/** 本机匿名账号的身份文件（`{subject, secret}`），会话过期或被拒以后靠它重新登录同一个匿名账号。 */
export const ANONYMOUS_IDENTITY_FILE: string = "anonymous-account.json";
export const ANONYMOUS_IDENTITY_ENTRY: string = "anonymous-account";

/** 云剪贴板经自建中继或 WebDAV 网盘时的设备身份（Rust 的 `RELAY_KEYSTORE_FILE`），与账号会话同在 `files/state`。 */
export const CLOUD_CLIPBOARD_RELAY_KEYSTORE_ENTRY: string = "cloud-clipboard-relay-keystore";
/** 云剪贴板经水杉账号时的设备身份（Rust 的 `CLOUD_CLIPBOARD_KEYSTORE_FILE`）。 */
export const CLOUD_CLIPBOARD_KEYSTORE_ENTRY: string = "cloud-clipboard-keystore";
/** WebDAV 网盘日志的本机缓存（Rust 的 `WEBDAV_STATE_FILE`），只有密文和公开数据。 */
export const CLOUD_CLIPBOARD_WEBDAV_STATE_ENTRY: string = "cloud-clipboard-webdav-state";

/**
 * 云剪贴板的设备身份只由设置应用生成，键盘扩展只用镜像过来的这一份（请求带 `create_identity: false`，本机没有就停下、不登记），同一台设备在空间里只是一台、只批准一次。
 *
 * 设置应用那边没有这份（还没用过云剪贴板）时不删键盘的副本：账号路径的旧版本里键盘自己登记过一台设备，在设置应用生成身份之前它照旧能用；设置应用的身份一到就整份换成它。退出登录时键盘的副本随会话一起由 Rust 的 `session_adopt` 清掉，不靠这里。
 */
export const CLOUD_CLIPBOARD_IDENTITY_ENTRIES: readonly string[] = [
  CLOUD_CLIPBOARD_RELAY_KEYSTORE_ENTRY,
  CLOUD_CLIPBOARD_KEYSTORE_ENTRY,
];

/** 设置应用发布、键盘导入的条目。 */
export const SETTINGS_ENTRIES: MirrorEntry[] = [
  {
    name: "dictionary-generations",
    kind: "directory",
    base: "state",
    path: "dictionary-updates/generations",
    fullModeOnly: false,
  },
  {
    name: "dictionary-selection",
    kind: "file",
    base: "state",
    path: "dictionary-updates/installed.json",
    fullModeOnly: false,
  },
  {
    name: "preferences",
    kind: "preferences",
    base: "state",
    path: "preferences.json",
    fullModeOnly: false,
  },
  {
    name: "key-feedback",
    kind: "file",
    base: "state",
    path: "key-feedback.json",
    fullModeOnly: false,
  },
  { name: "app-theme", kind: "file", base: "state", path: "app-theme.json", fullModeOnly: false },
  {
    name: "statistics-switch",
    kind: "file",
    base: "state",
    path: STATISTICS_SWITCH_FILE,
    fullModeOnly: false,
  },
  { name: "skins", kind: "directory", base: "state", path: "skins", fullModeOnly: false },
  { name: "plugins", kind: "directory", base: "state", path: "plugins", fullModeOnly: false },
  // 隐私政策的授权只由设置应用决定、只往键盘一个方向镜像；基础访问模式下键盘也要读到它，没读到就不联网。
  {
    name: PRIVACY_CONSENT_ENTRY,
    kind: "file",
    base: "state",
    path: PRIVACY_CONSENT_FILE,
    fullModeOnly: false,
  },
  // 「防止截屏」同样只由设置应用决定（切换要经系统认证）、只往键盘一个方向镜像；键盘在基础访问模式下也要读到它，给面板设隐私模式。不进 `KEYBOARD_ENTRIES`：键盘不能把它改回去。
  {
    name: SCREEN_CAPTURE_PROTECTION_ENTRY,
    kind: "file",
    base: "state",
    path: SCREEN_CAPTURE_PROTECTION_FILE,
    fullModeOnly: false,
  },
  {
    name: "community-library",
    kind: "file",
    base: "files",
    path: "CommunityLibrary.json",
    fullModeOnly: false,
  },
  {
    name: "account-session",
    kind: "file",
    base: "state",
    path: "account-session.json",
    fullModeOnly: true,
  },
  {
    name: ANONYMOUS_IDENTITY_ENTRY,
    kind: "file",
    base: "state",
    path: ANONYMOUS_IDENTITY_FILE,
    fullModeOnly: true,
  },
  {
    name: "anonymous-session",
    kind: "file",
    base: "state",
    path: "anonymous-session.json",
    fullModeOnly: true,
  },
  // 键盘从不改写中继的设备身份，只读得到就够了，所以基础访问模式下也导入。
  {
    name: CLOUD_CLIPBOARD_RELAY_KEYSTORE_ENTRY,
    kind: "file",
    base: "state",
    path: CLOUD_CLIPBOARD_RELAY_KEYSTORE_ENTRY,
    fullModeOnly: false,
  },
  // 网盘日志的缓存是同一台设备读到的同一份日志，键盘拿设置应用那份往后接着读就行。镜像它是为了「测试连接」通过后的重新信任能传到键盘：设置应用删掉自己的缓存，键盘的副本随之删掉，不会一直报 `webdav_rollback`。
  {
    name: CLOUD_CLIPBOARD_WEBDAV_STATE_ENTRY,
    kind: "file",
    base: "state",
    path: CLOUD_CLIPBOARD_WEBDAV_STATE_ENTRY,
    fullModeOnly: false,
  },
  // 账号路径的身份跟着账号会话走：基础访问模式下键盘不导入会话、没有登录，也用不到它。
  {
    name: CLOUD_CLIPBOARD_KEYSTORE_ENTRY,
    kind: "file",
    base: "state",
    path: CLOUD_CLIPBOARD_KEYSTORE_ENTRY,
    fullModeOnly: true,
  },
];

/** 键盘在完整体验模式下发布、设置应用导入的条目：键盘里改的偏好和按键反馈、刷新过的账号会话、匿名账号的身份（随匿名会话由 Rust 采用），以及打字统计。 */
export const KEYBOARD_ENTRIES: MirrorEntry[] = [
  {
    name: "preferences",
    kind: "preferences",
    base: "state",
    path: "preferences.json",
    fullModeOnly: true,
  },
  {
    name: "key-feedback",
    kind: "file",
    base: "state",
    path: "key-feedback.json",
    fullModeOnly: true,
  },
  {
    name: "account-session",
    kind: "file",
    base: "state",
    path: "account-session.json",
    fullModeOnly: true,
  },
  {
    name: ANONYMOUS_IDENTITY_ENTRY,
    kind: "file",
    base: "state",
    path: ANONYMOUS_IDENTITY_FILE,
    fullModeOnly: true,
  },
  {
    name: "anonymous-session",
    kind: "file",
    base: "state",
    path: "anonymous-session.json",
    fullModeOnly: true,
  },
  {
    name: "typing-statistics",
    kind: "file",
    base: "state",
    path: "typing-statistics.json",
    fullModeOnly: true,
  },
];

/** 由 Rust 的 `session_adopt` 采用、不按文件复制导入的条目：登录会话、本机匿名账号的会话（#8201 第 2 步），以及匿名账号的身份，它随匿名会话一起采用（`identity_path`，迁移差异 M40），会话和身份始终属于同一个匿名账号。 */
export const RUST_ADOPTED_ENTRIES: readonly string[] = [
  "account-session",
  "anonymous-session",
  ANONYMOUS_IDENTITY_ENTRY,
];

export class SharedStateMirrorPolicy {
  /** 这个条目是不是交给 Rust 采用：导入时调用方返回 `handled`，不复制也不删除本地文件。 */
  static adoptedByRust(name: string): boolean {
    return RUST_ADOPTED_ENTRIES.includes(name);
  }

  /** 发布方没有这一条时是否保留导入方自己的副本，而不是跟着删掉：只有云剪贴板的设备身份如此，见 `CLOUD_CLIPBOARD_IDENTITY_ENTRIES`。 */
  static keepsLocalWhenAbsent(name: string): boolean {
    return CLOUD_CLIPBOARD_IDENTITY_ENTRIES.includes(name);
  }

  /** 导入一个普通文件前的决定：两份会话和匿名账号的身份交给 Rust（`handled`，不复制也不删除本地文件），其余条目照常复制。 */
  static importDecision(name: string): ImportDecision {
    return SharedStateMirrorPolicy.adoptedByRust(name) ? "handled" : "copy";
  }

  /** FNV-1a 32 位加长度，只用来判断内容是否变了，不是安全用途。 */
  static fingerprintText(text: string): string {
    let hash: number = 0x811c9dc5;
    for (let index = 0; index < text.length; index++) {
      hash ^= text.charCodeAt(index);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    return `${text.length.toString(16)}-${hash.toString(16).padStart(8, "0")}`;
  }

  /** 指纹忽略修订号及各沙箱独立的授权元数据，避免本机代次差异造成镜像往返；其余偏好仍按内容比较。 */
  static preferencesFingerprint(documentText: string): string {
    try {
      const parsed: Object = JSON.parse(documentText) as Object;
      const preferences: Object | undefined = (parsed as PreferencesDocument).preferences;
      if (preferences !== undefined && preferences !== null) {
        return SharedStateMirrorPolicy.fingerprintText(
          SharedStateMirrorPolicy.canonical(
            SharedStateMirrorPolicy.withoutReportingConsent(preferences),
          ),
        );
      }
    } catch {
      // 落到下面按全文计算。
    }
    return SharedStateMirrorPolicy.fingerprintText(documentText);
  }

  private static withoutReportingConsent(preferences: Object): Object {
    const fields = preferences as Record<string, Object>;
    const result: Record<string, Object> = {};
    for (const key of Object.keys(fields))
      if (key !== "usage_reporting_consent") result[key] = fields[key];
    return result;
  }

  /** 设置应用导入键盘改动时保留自己的统计开关；键盘的陈旧副本不能替用户重新同意。 */
  static preferencesImport(
    documentText: string,
    localText: string,
    preserveReporting: boolean,
  ): string | null {
    try {
      const incoming: unknown = JSON.parse(documentText);
      const local: unknown = JSON.parse(localText);
      if (
        incoming === null ||
        typeof incoming !== "object" ||
        Array.isArray(incoming) ||
        local === null ||
        typeof local !== "object" ||
        Array.isArray(local)
      )
        return null;
      const source = (incoming as Record<string, unknown>).preferences;
      const current = (local as Record<string, unknown>).preferences;
      if (
        source === null ||
        typeof source !== "object" ||
        Array.isArray(source) ||
        current === null ||
        typeof current !== "object" ||
        Array.isArray(current)
      )
        return null;
      const preferences = SharedStateMirrorPolicy.withoutReportingConsent(source) as Record<
        string,
        unknown
      >;
      if (preserveReporting) {
        const enabled: unknown = (current as Record<string, unknown>).usage_reporting;
        if (typeof enabled !== "boolean") return null;
        preferences.usage_reporting = enabled;
      }
      const snapshot = local as Record<string, unknown>;
      snapshot.preferences = preferences;
      return JSON.stringify(snapshot);
    } catch {
      return null;
    }
  }

  /** 目录的指纹：每个文件的相对路径、大小和修改时间，按路径排序。空目录与不存在不同，前者也会被复制过去。 */
  static directoryFingerprint(files: ListedFile[]): string {
    const lines: string[] = files
      .map((file: ListedFile): string => `${file.path}\u0000${file.size}\u0000${file.mtime}`)
      .sort();
    return SharedStateMirrorPolicy.fingerprintText(`dir\u0001${lines.join("\u0001")}`);
  }

  /** 按这次算出的指纹更新发布清单：指纹变了的条目记下变化时刻，并列入要复制的名单。 */
  static plan(
    previous: MirrorManifest | null,
    publisher: MirrorPublisher,
    current: FingerprintedEntry[],
    nowMs: number,
    applied: ImportedEntry[],
  ): PublishPlan {
    const usable: MirrorManifest | null =
      previous !== null && previous.format === MIRROR_FORMAT && previous.publisher === publisher
        ? previous
        : null;
    const entries: ManifestEntry[] = [];
    const changed: string[] = [];
    for (const entry of current) {
      const before: ManifestEntry | undefined =
        usable === null
          ? undefined
          : usable.entries.find((item: ManifestEntry): boolean => item.name === entry.name);
      if (before !== undefined && before.fingerprint === entry.fingerprint) {
        entries.push({
          name: entry.name,
          fingerprint: entry.fingerprint,
          changedMs: before.changedMs,
        });
        continue;
      }
      // 从来没发布过、而且本来就不存在的条目不用通知对方删除。
      if (before === undefined && entry.fingerprint === ABSENT_FINGERPRINT) {
        continue;
      }
      entries.push({ name: entry.name, fingerprint: entry.fingerprint, changedMs: nowMs });
      changed.push(entry.name);
    }
    return {
      manifest: { format: MIRROR_FORMAT, publisher: publisher, entries: entries, applied: applied },
      changed: changed,
    };
  }

  /**
   * 该从对方导入哪些条目。
   *
   * 对方的指纹与上次导入的不同才导入；如果这一侧在对方那次变化之后自己又改过（自己的发布清单里同一条的变化时刻更晚），以这一侧为准，不导入。键盘在基础访问模式下不发布，传入 `own` 为 null，于是设置应用的改动总会覆盖键盘里的同一条。`fullMode` 为假时跳过只在完整体验模式下交换的条目。
   */
  static pending(
    theirs: MirrorManifest | null,
    record: ImportRecord | null,
    own: MirrorManifest | null,
    entries: MirrorEntry[],
    fullMode: boolean,
  ): ManifestEntry[] {
    if (theirs === null || theirs.format !== MIRROR_FORMAT) {
      return [];
    }
    const result: ManifestEntry[] = [];
    for (const entry of entries) {
      if (entry.fullModeOnly && !fullMode) continue;
      const published: ManifestEntry | undefined = theirs.entries.find(
        (item: ManifestEntry): boolean => item.name === entry.name,
      );
      if (published === undefined) continue;
      const imported: ImportedEntry | undefined =
        record === null
          ? undefined
          : record.entries.find((item: ImportedEntry): boolean => item.name === entry.name);
      if (imported !== undefined && imported.fingerprint === published.fingerprint) continue;
      const mine: ManifestEntry | undefined =
        own === null
          ? undefined
          : own.entries.find((item: ManifestEntry): boolean => item.name === entry.name);
      if (
        mine !== undefined &&
        mine.fingerprint !== published.fingerprint &&
        mine.changedMs > published.changedMs
      ) {
        continue;
      }
      result.push(published);
    }
    return result;
  }

  /** 记下某条已经导入到哪一版。 */
  static recordImport(
    record: ImportRecord | null,
    name: string,
    fingerprint: string,
  ): ImportRecord {
    const entries: ImportedEntry[] =
      record === null
        ? []
        : record.entries.filter((item: ImportedEntry): boolean => item.name !== name);
    entries.push({ name: name, fingerprint: fingerprint });
    return { format: MIRROR_FORMAT, entries: entries };
  }

  /**
   * 设置应用能否采用键盘发布的打字统计：键盘必须已经导入了设置应用当前的统计开关。否则键盘的文件还带着旧的开关或保留期，导入会把用户刚改的设置改回去。
   */
  static statisticsInStep(
    keyboard: MirrorManifest | null,
    settingsSwitchFingerprint: string,
  ): boolean {
    if (keyboard === null || settingsSwitchFingerprint.length === 0) return false;
    const applied: ImportedEntry | undefined = keyboard.applied.find(
      (item: ImportedEntry): boolean => item.name === "statistics-switch",
    );
    return applied !== undefined && applied.fingerprint === settingsSwitchFingerprint;
  }

  /**
   * 设置应用采用键盘的打字统计时合并两份文件：计数、开关和保留期取键盘的；试过的皮肤（`skinsTried`）和已解锁的徽章（`achievements`）是设置应用记的，键盘那份没有，取两边的并集，同一个徽章保留较早的解锁日。键盘的文件读不懂时返回 null，这一条下次再试。
   */
  static mergeStatistics(keyboardText: string, settingsText: string | null): string | null {
    let keyboard: Record<string, Object>;
    try {
      const parsed: Object = JSON.parse(keyboardText) as Object;
      if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
      keyboard = parsed as Record<string, Object>;
    } catch {
      return null;
    }
    let settings: Record<string, Object> = {};
    if (settingsText !== null) {
      try {
        const parsed: Object = JSON.parse(settingsText) as Object;
        if (parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)) {
          settings = parsed as Record<string, Object>;
        }
      } catch {
        // 设置应用自己那份读不懂时，只用键盘的。
      }
    }
    const skins: string[] = SharedStateMirrorPolicy.strings(keyboard["skinsTried"]).concat(
      SharedStateMirrorPolicy.strings(settings["skinsTried"]),
    );
    const uniqueSkins: string[] = skins
      .filter((item: string, index: number): boolean => skins.indexOf(item) === index)
      .sort();
    if (uniqueSkins.length > 0) {
      keyboard["skinsTried"] = uniqueSkins;
    }
    const achievements: Record<string, Object> = {};
    for (const source of [keyboard["achievements"], settings["achievements"]]) {
      if (
        source === undefined ||
        source === null ||
        typeof source !== "object" ||
        Array.isArray(source)
      )
        continue;
      const map: Record<string, Object> = source as Record<string, Object>;
      for (const id of Object.keys(map)) {
        const day: Object = map[id];
        if (typeof day !== "string") continue;
        const known: Object | undefined = achievements[id];
        if (known === undefined || (day as string) < (known as string)) {
          achievements[id] = day;
        }
      }
    }
    if (Object.keys(achievements).length > 0) {
      const sorted: Record<string, Object> = {};
      for (const id of Object.keys(achievements).sort()) sorted[id] = achievements[id];
      keyboard["achievements"] = sorted;
    }
    return JSON.stringify(keyboard);
  }

  private static strings(value: Object | undefined): string[] {
    if (!Array.isArray(value)) return [];
    return (value as Object[]).filter(
      (item: Object): boolean => typeof item === "string",
    ) as string[];
  }

  static parseManifest(text: string): MirrorManifest | null {
    try {
      const parsed: MirrorManifest = JSON.parse(text) as MirrorManifest;
      if (parsed.format !== MIRROR_FORMAT || !Array.isArray(parsed.entries)) return null;
      if (parsed.publisher !== "settings" && parsed.publisher !== "keyboard") return null;
      const entries: ManifestEntry[] = parsed.entries.filter(
        (item: ManifestEntry): boolean =>
          typeof item.name === "string" &&
          typeof item.fingerprint === "string" &&
          typeof item.changedMs === "number",
      );
      const applied: ImportedEntry[] = Array.isArray(parsed.applied)
        ? parsed.applied.filter(
            (item: ImportedEntry): boolean =>
              typeof item.name === "string" && typeof item.fingerprint === "string",
          )
        : [];
      return {
        format: MIRROR_FORMAT,
        publisher: parsed.publisher,
        entries: entries,
        applied: applied,
      };
    } catch {
      return null;
    }
  }

  static parseRecord(text: string): ImportRecord | null {
    try {
      const parsed: ImportRecord = JSON.parse(text) as ImportRecord;
      if (parsed.format !== MIRROR_FORMAT || !Array.isArray(parsed.entries)) return null;
      return {
        format: MIRROR_FORMAT,
        entries: parsed.entries.filter(
          (item: ImportedEntry): boolean =>
            typeof item.name === "string" && typeof item.fingerprint === "string",
        ),
      };
    } catch {
      return null;
    }
  }

  static parseStatisticsSwitch(text: string): StatisticsSwitch | null {
    try {
      const parsed: StatisticsSwitch = JSON.parse(text) as StatisticsSwitch;
      if (
        typeof parsed.enabled !== "boolean" ||
        !RETENTIONS.includes(parsed.retention) ||
        typeof parsed.resets !== "number" ||
        !Number.isInteger(parsed.resets) ||
        parsed.resets < 0
      ) {
        return null;
      }
      return { enabled: parsed.enabled, retention: parsed.retention, resets: parsed.resets };
    } catch {
      return null;
    }
  }

  private static canonical(value: Object | null): string {
    if (value === null || typeof value !== "object") {
      return JSON.stringify(value);
    }
    if (Array.isArray(value)) {
      return `[${(value as Object[]).map((item: Object): string => SharedStateMirrorPolicy.canonical(item)).join(",")}]`;
    }
    const keys: string[] = Object.keys(value).sort();
    const parts: string[] = keys.map(
      (key: string): string =>
        `${JSON.stringify(key)}:${SharedStateMirrorPolicy.canonical((value as Record<string, Object>)[key])}`,
    );
    return `{${parts.join(",")}}`;
  }
}

interface PreferencesDocument {
  preferences?: Object;
}

interface CachedFingerprint {
  stamp: string;
  fingerprint: string;
}

/**
 * 发布方每次发布都要算一遍本地条目的内容指纹：读整个文件、偏好还要解析后按键排序，再逐字符算 FNV。键盘在完整体验模式下每次失焦、获得焦点各发布一次，中间文件大多没动，打字统计开着时这份文件又会越攒越大，于是每次切换输入框都在 UI 线程上把同样的内容读两遍、算两遍。
 *
 * 这里按路径记住上次算出的指纹和当时文件的身份（inode、大小、纳秒修改时间），身份没变就直接用上次的指纹，不读文件。指纹本身仍是内容指纹，导入写回同样内容时指纹也相同，不会来回打转。写这些文件的一方（client-core 的存储、`writePrivateText`、镜像自己的 `copy`）都是先写暂存文件再改名换入，每次写 inode 都变；拿不到纳秒修改时间（`stamp` 为 null）时不缓存，照旧每次读。读不到文件（指纹是 `ABSENT_FINGERPRINT`）时也不记，下次再读。缓存只在本进程内，设置应用和键盘各有各的。
 */
export class FingerprintCache {
  private readonly entries: Map<string, CachedFingerprint> = new Map<string, CachedFingerprint>();

  /** `stamp` 与上次记下的相同时返回上次的指纹，否则调用 `compute` 重新算并记下。 */
  fingerprint(key: string, stamp: string | null, compute: () => string): string {
    if (stamp === null) {
      this.entries.delete(key);
      return compute();
    }
    const cached: CachedFingerprint | undefined = this.entries.get(key);
    if (cached !== undefined && cached.stamp === stamp) {
      return cached.fingerprint;
    }
    const fingerprint: string = compute();
    if (fingerprint === ABSENT_FINGERPRINT) {
      this.entries.delete(key);
    } else {
      this.entries.set(key, { stamp: stamp, fingerprint: fingerprint });
    }
    return fingerprint;
  }

  /** 一个文件的身份：inode、大小和纳秒修改时间。没有纳秒修改时间时为 null，秒级的修改时间分不清同一秒里的两次改写。 */
  static stamp(ino: string, size: number, mtimeNs: string | undefined): string | null {
    return mtimeNs === undefined ? null : `${ino}:${size}:${mtimeNs}`;
  }
}
