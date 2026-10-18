/**
 * 设置应用和键盘经共享沙箱交换哪些状态、什么时候发布、什么时候导入。
 *
 * 两个进程各自保留私有的 `files/state`，原生层照旧从它推出偏好、`user/`、`cache/`。共享沙箱（`context.getGroupDir`）只用来交换对方要看到的那部分：发布方把文件复制到 `<共享目录>/<发布方>/`，最后写一份清单；导入方读清单，把指纹与上次导入不同的条目复制回自己的私有目录。不让两边直接共用一个状态目录，是因为 client-core 的存储连读都要在目录里建锁文件加锁，而基础访问模式下键盘对共享目录只读，直接共用时键盘连偏好都读不了；`prepare_host` 每次也要往状态目录里写。
 *
 * 指纹只看内容：偏好按 `preferences` 本身（不含修订号），小文件按全文，目录按每个文件的路径、大小和修改时间。导入写回的内容与对方相同，指纹也相同，所以不会再被发布回去，两边不会来回打转。
 */

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
   * 只在键盘处于完整体验模式时交换。账号会话属于这一类：服务端会轮换刷新令牌，同一个刷新令牌用第二次会被拒绝，所以两边持有同一份会话时必须都能把刷新后的会话写回；基础访问模式下键盘写不了共享目录，就不导入，继续用自己的匿名账号。
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
    name: "anonymous-account",
    kind: "file",
    base: "state",
    path: "anonymous-account.json",
    fullModeOnly: true,
  },
  {
    name: "anonymous-session",
    kind: "file",
    base: "state",
    path: "anonymous-session.json",
    fullModeOnly: true,
  },
];

/** 键盘在完整体验模式下发布、设置应用导入的条目：键盘里改的偏好和按键反馈、刷新过的账号会话，以及打字统计。 */
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

export class SharedStateMirrorPolicy {
  /** FNV-1a 32 位加长度，只用来判断内容是否变了，不是安全用途。 */
  static fingerprintText(text: string): string {
    let hash: number = 0x811c9dc5;
    for (let index = 0; index < text.length; index++) {
      hash ^= text.charCodeAt(index);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    return `${text.length.toString(16)}-${hash.toString(16).padStart(8, "0")}`;
  }

  /** 偏好文档的指纹只看 `preferences`，键按字典序排好再算，修订号和键的顺序都不影响它。读不懂的文档按全文算，至少能在它变化时察觉。 */
  static preferencesFingerprint(documentText: string): string {
    try {
      const parsed: Object = JSON.parse(documentText) as Object;
      const preferences: Object | undefined = (parsed as PreferencesDocument).preferences;
      if (preferences !== undefined && preferences !== null) {
        return SharedStateMirrorPolicy.fingerprintText(
          SharedStateMirrorPolicy.canonical(preferences),
        );
      }
    } catch {
      // 落到下面按全文计算。
    }
    return SharedStateMirrorPolicy.fingerprintText(documentText);
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
