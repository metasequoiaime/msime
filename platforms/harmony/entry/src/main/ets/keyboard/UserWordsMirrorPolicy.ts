/**
 * 设置应用和键盘的「我的词条」只有一份：两个进程各自的 Engine 日志经共享沙箱里的发布文件交换，规则（每个词条一个「最后写入者胜」的寄存器、第一次合并取并集、大批删除挡下、墓碑在对方确认后回收）都在 Rust 的 `msime_client_user_words_mirror` 里，见 C 头文件。这里只决定请求长什么样、什么时候发、回复怎么读。
 *
 * - 设置应用：窗口回到前台和每次改完设置之后（与镜像的发布同一个时机）在工作线程上 `sync`：合并键盘发布的改动、发布自己的。它的日志和键盘的发布都没变时不发。键盘的改动写进来以后，词库同步要记一次本机改动（`local_change`），网盘才会收到键盘学到的词。
 * - 键盘：写本机词库要 Engine 的独占维护权，只有建会话之前拿得到，所以 `sync` 放在建会话之前；获得焦点时发现设置应用的发布变了，就排一次空闲时的会话重建。会话开着时只能 `publish`（只记录、不写词库），失去焦点时在工作线程上做，基础访问模式下键盘写不了共享目录，不发布。
 */

/** 本方私有目录：`files/state` 下的这一项，放状态和锁。 */
export const USER_WORDS_DIRECTORY: string = "user-words";
/** 发布文件相对 `<共享目录>/msime/<发布方>/` 的路径。 */
export const USER_WORDS_PUBLICATION: string = "user-words/words.json";
/** 键盘建会话之前最多写几项：这一段在主线程上，剩下的在空闲时的下一次会话重建里接着写。 */
export const KEYBOARD_MAX_CHANGES: number = 128;
/** 设置应用一次最多写几项：在工作线程上，但设置页的词库操作要和它抢同一把维护权，所以也不一次写完。 */
export const SETTINGS_MAX_CHANGES: number = 512;
/** 键盘建会话之前的 `sync` 连续失败几次就不再在空闲时重建会话重试，等设置应用的下一次发布：每次重试都要重建一次 Engine 会话。 */
export const KEYBOARD_SYNC_ATTEMPTS: number = 3;
/** 键盘两次 `publish` 至少隔这么久：每次都要读整份日志，失去焦点又很频繁。 */
export const KEYBOARD_PUBLISH_INTERVAL_MS: number = 30000;
/** 设置应用这一次没写完（忙或者到了上限）时，隔这么久再试。 */
export const SETTINGS_RETRY_MS: number = 5000;
/** 设置应用一次最多排空几轮个人词库队列：每轮最多写 4 条（队列的上限），剩下的下次接着写。 */
export const SETTINGS_QUEUE_ROUNDS: number = 64;

export type UserWordsSide = "settings" | "keyboard";
export type UserWordsOperation = "sync" | "publish";

/** 一个文件此刻的身份，用来判断它变没变：大小、修改时间和 inode。 */
export interface FileIdentity {
  readonly size: number;
  readonly mtime: number;
  /** `fs.Stat.ino` 是 bigint，转成十进制字符串。 */
  readonly ino: string;
}

export interface UserWordsReply {
  ok: boolean;
  error: string;
  captured: number;
  applied: number;
  rejected: number;
  deferred: number;
  heldDeletions: number;
  words: number;
  theirs: string;
  published: boolean;
}

export interface KeyboardSyncOutcome {
  settled: boolean;
  failures: number;
}

/** 稍后再试就可能成功的错误：另一次调用正占着本方目录、读写词库或文件一时失败。 */
const RETRYABLE_ERRORS: readonly string[] = [
  "user_words_busy",
  "user_words_unavailable",
  "user_words_storage",
];

export class UserWordsMirrorPolicy {
  /** 本方私有目录。 */
  static directory(stateDirectory: string): string {
    return `${stateDirectory}/${USER_WORDS_DIRECTORY}`;
  }

  /** 某一方在共享目录里的发布文件；`groupRoot` 是 `<共享目录>/msime`。 */
  static publication(groupRoot: string, publisher: UserWordsSide): string {
    return `${groupRoot}/${publisher}/${USER_WORDS_PUBLICATION}`;
  }

  /** 本方的 Engine 日志：`prepareHost` 把 `user_data` 放在 `files/state/user` 下，日志就是那里的 `msime_user.db`，WAL 模式下改动先落在 `-wal` 里。 */
  static journalFiles(stateDirectory: string): string[] {
    const journal: string = `${stateDirectory}/user/msime_user.db`;
    return [journal, `${journal}-wal`];
  }

  /** 设置应用排给个人词库队列的词所在的文件：队列本身和命名词库的待发送队列。它们变了，即使日志没变也要排空一次。 */
  static settingsQueueFiles(stateDirectory: string): string[] {
    return [
      `${stateDirectory}/PersonalDictionary/sync.json`,
      `${stateDirectory}/DictionaryCollections/index.json`,
      `${stateDirectory}/DictionaryCollections/outbox.json`,
    ];
  }

  /** 几个文件的身份拼成一个标记，任何一个变了标记就变；不存在的文件记成 `-`。 */
  static mark(identities: (FileIdentity | null)[]): string {
    return identities
      .map((identity: FileIdentity | null): string =>
        identity === null ? "-" : `${identity.size}:${identity.mtime}:${identity.ino}`,
      )
      .join("|");
  }

  /**
   * `msime_client_user_words_mirror` 的请求。`optionsDocument` 是 `prepareHost` 给的 HostOptions 文档，原样嵌进去，免得为了拼请求把一份带着皮肤照片的偏好文档再解析一遍。`publish` 为 null 时不发布（键盘在基础访问模式下）。
   */
  static request(
    operation: UserWordsOperation,
    side: UserWordsSide,
    optionsDocument: string,
    stateDirectory: string,
    theirs: string | null,
    publish: string | null,
    maxChanges: number,
  ): string {
    const fields: string[] = [
      `"operation":${JSON.stringify(operation)}`,
      `"side":${JSON.stringify(side)}`,
      `"options":${optionsDocument}`,
      `"directory":${JSON.stringify(UserWordsMirrorPolicy.directory(stateDirectory))}`,
    ];
    if (operation === "sync" && theirs !== null) {
      fields.push(`"theirs":${JSON.stringify(theirs)}`);
    }
    if (publish !== null) {
      fields.push(`"publish":${JSON.stringify(publish)}`);
    }
    if (operation === "sync") {
      fields.push(`"max_changes":${Math.max(1, Math.floor(maxChanges))}`);
    }
    return `{${fields.join(",")}}`;
  }

  /** 读回复。读不懂时按存储错误处理：下次再试。 */
  static parse(text: string): UserWordsReply {
    const failed = (error: string): UserWordsReply => ({
      ok: false,
      error: error,
      captured: 0,
      applied: 0,
      rejected: 0,
      deferred: 0,
      heldDeletions: 0,
      words: 0,
      theirs: "",
      published: false,
    });
    try {
      const parsed: Record<string, Object> = JSON.parse(text) as Record<string, Object>;
      if (parsed["ok"] !== true) {
        const error: Object | undefined = parsed["error"];
        return failed(typeof error === "string" ? error : "user_words_storage");
      }
      const value: Record<string, Object> = parsed["value"] as Record<string, Object>;
      const count = (name: string): number => {
        const field: Object | undefined = value[name];
        return typeof field === "number" && Number.isInteger(field) && field >= 0 ? field : 0;
      };
      const theirs: Object | undefined = value["theirs"];
      return {
        ok: true,
        error: "",
        captured: count("captured"),
        applied: count("applied"),
        rejected: count("rejected"),
        deferred: count("deferred"),
        heldDeletions: count("held_deletions"),
        words: count("words"),
        theirs: typeof theirs === "string" ? theirs : "",
        published: value["published"] === true,
      };
    } catch {
      return failed("user_words_storage");
    }
  }

  /** 这一次做完了：成功并且没有留到下一次的，或者是重试也没用的错误（请求不合法、超过上限）。没做完时调用方安排下一次。 */
  static settled(reply: UserWordsReply): boolean {
    if (reply.ok) return reply.deferred === 0;
    return !RETRYABLE_ERRORS.includes(reply.error);
  }

  /** 键盘这一次 `sync` 之后：算不算做完，以及连续失败了几次。有进展（成功，哪怕还有留到下一次的）就把失败次数清零；连续失败到 `KEYBOARD_SYNC_ATTEMPTS` 次就算做完，免得状态目录写不进、词库读不出时每两秒重建一次会话。 */
  static keyboardOutcome(reply: UserWordsReply, failures: number): KeyboardSyncOutcome {
    if (reply.ok) return { settled: reply.deferred === 0, failures: 0 };
    if (UserWordsMirrorPolicy.settled(reply)) return { settled: true, failures: 0 };
    const next: number = failures + 1;
    return {
      settled: next >= KEYBOARD_SYNC_ATTEMPTS,
      failures: next >= KEYBOARD_SYNC_ATTEMPTS ? 0 : next,
    };
  }

  /** 键盘在建会话之前要不要 `sync`：本进程还没做过、上次没做完，或者设置应用的发布从上次做完以后变了。 */
  static keyboardSyncDue(due: boolean, mergedMark: string | null, currentMark: string): boolean {
    return due || mergedMark === null || mergedMark !== currentMark;
  }

  /** 键盘失去焦点时要不要 `publish`：完整体验模式、日志从上次发布以后变了、距上次发布够久，而且上一次还没在进行。 */
  static keyboardPublishDue(
    fullMode: boolean,
    publishing: boolean,
    publishedJournal: string | null,
    currentJournal: string,
    lastPublishMs: number,
    nowMs: number,
  ): boolean {
    if (!fullMode || publishing) return false;
    if (publishedJournal !== null && publishedJournal === currentJournal) return false;
    return lastPublishMs === 0 || nowMs - lastPublishMs >= KEYBOARD_PUBLISH_INTERVAL_MS;
  }

  /** 设置应用要不要 `sync`：自己的日志或者键盘的发布从上次做完以后变了（第一次总要做）。 */
  static settingsSyncDue(previous: string | null, current: string): boolean {
    return previous === null || previous !== current;
  }

  /** 一轮排空之后队列空了没有：个人词库队列没有待处理的，命名词库这次也没再送来新的一批。读不懂的回复按没空处理，下次再试。 */
  static queueDrained(personalReply: string, flushReply: string): boolean {
    try {
      const personal: Record<string, Object> = JSON.parse(personalReply) as Record<string, Object>;
      const flushed: Record<string, Object> = JSON.parse(flushReply) as Record<string, Object>;
      if (personal["ok"] !== true) return false;
      const pending: Object | undefined = (personal["value"] as Record<string, Object>)[
        "pending_count"
      ];
      // 没有命名词库时 flush 答不出来也不算没空：那里本来就没有要送的。
      const sent: Object | undefined =
        flushed["ok"] === true ? (flushed["value"] as Record<string, Object>)["sent"] : 0;
      return pending === 0 && (sent === undefined || sent === 0);
    } catch {
      return false;
    }
  }

  /**
   * 设置应用的词库这一次有了词库同步还没记过的改动：键盘的改动写了进来（`applied`），或者本方日志里有了上次以来的新改动（`captured`，其中设置应用自己排空队列写进来的词，页面在排队时通知过一次，那时它们还不在词库里）。词库同步要再记一次本机改动，网盘才收得到；页面自己改的词也在其中，多记一次不会多发。全被 Engine 拒绝的、失败的不算。
   */
  static dictionaryChanged(reply: UserWordsReply): boolean {
    return reply.ok && (reply.applied > 0 || reply.captured > 0);
  }
}
