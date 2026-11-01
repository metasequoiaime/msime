/**
 * 「加入生词本」在鸿蒙上的两头：键盘写进自己沙箱里的一本，设置应用导入键盘发布的那本时并进自己的生词本，键盘再把设置应用确认并过的词清走。
 *
 * API 12 起输入法扩展用独立沙箱，键盘写不到设置应用的目录，背单词又在设置应用里运行。所以键盘把词写进自己 `files/state` 下的 `keyboard-vocabulary/`，完整体验模式下经共享沙箱镜像发布（`SharedStateMirrorPolicy` 的 `personal-wordbook` 条目），设置应用导入时调 `msime_client_personal_wordbook_merge`，只并进上次以后新出现的词条。设置应用在自己的发布清单 `applied` 里写明并到了哪一版；键盘看到它与自己发布的那一版相同，就调 `msime_client_personal_wordbook_drain` 把那一版里的词清走，并记一次清走（`personal-wordbook-drained` 条目），设置应用看到以后不再拿上一版去比。键盘那本因此只是一个待交的队列。键盘那本不放在背单词的目录里：导入方按同一个相对路径存下上次并过的那一版，放在那里就会盖掉设置应用自己的生词本。
 *
 * 规则（释义怎么变成卡片、怎么合并、提示怎么说）都在共享层，这里只拼请求、读回答。
 */

/** 键盘那本生词本所在的目录，相对 `files/state`。 */
export const KEYBOARD_VOCABULARY_DIRECTORY: string = "keyboard-vocabulary";
/** 键盘那本生词本的文件，相对 `files/state`；镜像条目和设置应用存的「上次并过的那一版」都用这个路径。 */
export const KEYBOARD_PERSONAL_WORDBOOK_PATH: string = `${KEYBOARD_VOCABULARY_DIRECTORY}/vocabulary-wordbooks/personal.json`;
/** 键盘记的「清走过哪一版」，相对 `files/state`：`{drains, fingerprint}`，每清走一次 `drains` 加一，内容因此每次都不同，镜像一定会把它交过去。只有指纹和次数，没有词。设置应用按同一个相对路径存下它用过的最后一条。 */
export const KEYBOARD_PERSONAL_WORDBOOK_DRAINED_PATH: string = `${KEYBOARD_VOCABULARY_DIRECTORY}/drained.json`;
/** 交不出去或存储出错时的提示；其余提示都由共享层给出。 */
export const PERSONAL_WORDBOOK_FAILURE: string = "无法加入生词本";
/** 基础访问模式下键盘对共享目录只读，写进去了的词要等开启完整体验模式才交得过去。 */
export const PERSONAL_WORDBOOK_BASIC_MODE_HINT: string = "，开启完整体验模式后在背单词里可见";
/** 没有共享沙箱（模拟器，或签名不带 data-group-id）时，开启完整体验模式也交不过去。 */
export const PERSONAL_WORDBOOK_UNSHARED_HINT: string = "，仅保存在键盘里，暂时无法同步到背单词";

/** 键盘写进去的词能不能交给设置应用。 */
export type PersonalWordbookRoute = "settings" | "basic_mode" | "no_shared_sandbox";

/** 键盘清走过的那一版。 */
export interface DrainedMarker {
  drains: number;
  fingerprint: string;
}

interface AddRequest {
  directory: string;
  candidate: string;
  gloss: string;
}

interface MergeRequest {
  directory: string;
  published: string;
  previous?: string;
}

interface DrainRequest {
  directory: string;
  published: string;
}

interface Envelope {
  ok?: boolean;
  value?: NoticeValue;
}

interface MergeEnvelope {
  ok?: boolean;
  value?: MergeValue;
}

interface MergeValue {
  overLimit?: number;
}

interface DrainEnvelope {
  ok?: boolean;
  value?: DrainValue;
}

interface DrainValue {
  removed?: number;
}

interface NoticeValue {
  notice?: string;
  outcome?: string;
}

export class PersonalWordbookPolicy {
  /** 键盘的 `msime_client_personal_wordbook_add` 请求：写进键盘沙箱 `stateDirectory` 下的 `keyboard-vocabulary/`。 */
  static addRequest(stateDirectory: string, candidate: string, gloss: string): string {
    const request: AddRequest = {
      directory: `${stateDirectory}/${KEYBOARD_VOCABULARY_DIRECTORY}`,
      candidate: candidate,
      gloss: gloss,
    };
    return JSON.stringify(request);
  }

  /**
   * 回答里给用户看的那句提示。写进去了的结果（加入、合并、已经在）到不了设置应用时补一句：基础访问模式下说开启完整体验模式后可见，没有共享沙箱时说暂时无法同步。没写进去的结果（已满、释义不能用）照原样。
   */
  static notice(response: string, route: PersonalWordbookRoute): string {
    let envelope: Envelope;
    try {
      envelope = JSON.parse(response) as Envelope;
    } catch {
      return PERSONAL_WORDBOOK_FAILURE;
    }
    const value: NoticeValue | undefined = envelope.ok === true ? envelope.value : undefined;
    const notice: string | undefined = value === undefined ? undefined : value.notice;
    if (typeof notice !== "string" || notice.length === 0) return PERSONAL_WORDBOOK_FAILURE;
    const stored: boolean =
      value !== undefined &&
      (value.outcome === "added" ||
        value.outcome === "merged" ||
        value.outcome === "already_present");
    if (!stored || route === "settings") return notice;
    return route === "basic_mode"
      ? `${notice}${PERSONAL_WORDBOOK_BASIC_MODE_HINT}`
      : `${notice}${PERSONAL_WORDBOOK_UNSHARED_HINT}`;
  }

  /** 键盘这次写进去的词能交到哪里：没有共享沙箱、基础访问模式，还是能交给设置应用。 */
  static route(hasSharedSandbox: boolean, fullMode: boolean): PersonalWordbookRoute {
    if (!hasSharedSandbox) return "no_shared_sandbox";
    return fullMode ? "settings" : "basic_mode";
  }

  /** 设置应用的 `msime_client_personal_wordbook_merge` 请求，都是绝对路径。`previous` 是上次并过的那一版，没有时为 null。 */
  static mergeRequest(stateDirectory: string, published: string, previous: string | null): string {
    const request: MergeRequest = { directory: stateDirectory, published: published };
    if (previous !== null) request.previous = previous;
    return JSON.stringify(request);
  }

  /**
   * 能不能把这一版记成已经并过。合并失败、或者设置应用那本已满有词没并进来（`overLimit` 大于 0）时都不能：记成并过以后键盘会把这些词清走，用户腾出地方也回不来了，所以下次再试。
   */
  static mergeAccepted(response: string): boolean {
    try {
      const envelope: MergeEnvelope = JSON.parse(response) as MergeEnvelope;
      if (envelope.ok !== true || envelope.value === undefined || envelope.value === null)
        return false;
      const overLimit: number | undefined = envelope.value.overLimit;
      return typeof overLimit === "number" && overLimit === 0;
    } catch {
      return false;
    }
  }

  /** 键盘的 `msime_client_personal_wordbook_drain` 请求：从键盘沙箱 `stateDirectory` 下的 `keyboard-vocabulary/` 里清走 `published`（共享目录里键盘发布的那份，绝对路径）里的词。 */
  static drainRequest(stateDirectory: string, published: string): string {
    const request: DrainRequest = {
      directory: `${stateDirectory}/${KEYBOARD_VOCABULARY_DIRECTORY}`,
      published: published,
    };
    return JSON.stringify(request);
  }

  /** 清走了几个词；请求失败时为 -1。 */
  static drained(response: string): number {
    try {
      const envelope: DrainEnvelope = JSON.parse(response) as DrainEnvelope;
      if (envelope.ok !== true || envelope.value === undefined || envelope.value === null)
        return -1;
      const removed: number | undefined = envelope.value.removed;
      return typeof removed === "number" && removed >= 0 ? removed : -1;
    } catch {
      return -1;
    }
  }

  static parseDrainedMarker(text: string | null): DrainedMarker | null {
    if (text === null) return null;
    try {
      const parsed: DrainedMarker = JSON.parse(text) as DrainedMarker;
      if (
        parsed === null ||
        typeof parsed !== "object" ||
        typeof parsed.drains !== "number" ||
        !Number.isInteger(parsed.drains) ||
        parsed.drains < 0 ||
        typeof parsed.fingerprint !== "string" ||
        parsed.fingerprint.length === 0
      ) {
        return null;
      }
      return { drains: parsed.drains, fingerprint: parsed.fingerprint };
    } catch {
      return null;
    }
  }

  /** 又清走了指纹为 `fingerprint` 的那一版以后要写的记录。`previous` 是原来那份记录的内容，没有或读不懂时从零数起。 */
  static nextDrainedMarker(previous: string | null, fingerprint: string): string {
    const marker: DrainedMarker | null = PersonalWordbookPolicy.parseDrainedMarker(previous);
    const next: DrainedMarker = {
      drains: marker === null ? 1 : marker.drains + 1,
      fingerprint: fingerprint,
    };
    return JSON.stringify(next);
  }

  /** 设置应用一侧：键盘发布的清走记录 `markerText` 是不是一条还没用过的新记录。`seenText` 是上次用过的那条，没有时为 null。读不懂的记录不算。 */
  static unseenDrain(markerText: string | null, seenText: string | null): boolean {
    return (
      PersonalWordbookPolicy.parseDrainedMarker(markerText) !== null && markerText !== seenText
    );
  }

  /** 设置应用一侧：键盘的记录说它已经清走了设置应用存下的上一版（指纹为 `previousFingerprint`）。这时键盘那本里的词都是之后加的，不该再拿上一版去比。 */
  static previousDrained(markerText: string | null, previousFingerprint: string): boolean {
    const marker: DrainedMarker | null = PersonalWordbookPolicy.parseDrainedMarker(markerText);
    return marker !== null && marker.fingerprint === previousFingerprint;
  }
}
