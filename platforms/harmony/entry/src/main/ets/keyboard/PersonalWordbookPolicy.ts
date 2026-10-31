/**
 * 「加入生词本」在鸿蒙上的两头：键盘写进自己沙箱里的一本，设置应用导入键盘发布的那本时并进自己的生词本。
 *
 * API 12 起输入法扩展用独立沙箱，键盘写不到设置应用的目录，背单词又在设置应用里运行。所以键盘把词写进自己 `files/state` 下的 `keyboard-vocabulary/`，完整体验模式下经共享沙箱镜像发布（`SharedStateMirrorPolicy` 的 `personal-wordbook` 条目），设置应用导入时调 `msime_client_personal_wordbook_merge`，只并进上次以后新出现的词条。键盘那本不放在背单词的目录里：导入方按同一个相对路径存下上次并过的那一版，放在那里就会盖掉设置应用自己的生词本。
 *
 * 规则（释义怎么变成卡片、怎么合并、提示怎么说）都在共享层，这里只拼请求、读回答。
 */

/** 键盘那本生词本所在的目录，相对 `files/state`。 */
export const KEYBOARD_VOCABULARY_DIRECTORY: string = "keyboard-vocabulary";
/** 键盘那本生词本的文件，相对 `files/state`；镜像条目和设置应用存的「上次并过的那一版」都用这个路径。 */
export const KEYBOARD_PERSONAL_WORDBOOK_PATH: string = `${KEYBOARD_VOCABULARY_DIRECTORY}/vocabulary-wordbooks/personal.json`;
/** 交不出去或存储出错时的提示；其余提示都由共享层给出。 */
export const PERSONAL_WORDBOOK_FAILURE: string = "无法加入生词本";
/** 键盘写进去了、但还到不了设置应用时补的一句：没有共享沙箱，或者基础访问模式下键盘对共享目录只读。 */
export const PERSONAL_WORDBOOK_UNSHARED_HINT: string = "，开启完整体验模式后在背单词里可见";

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

interface Envelope {
  ok?: boolean;
  value?: NoticeValue;
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
   * 回答里给用户看的那句提示。`reachesSettings` 为假（没有共享沙箱或基础访问模式）时，写进去了的结果补一句什么时候能在背单词里看到；没写进去的结果（已满、释义不能用）照原样。
   */
  static notice(response: string, reachesSettings: boolean): string {
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
    return stored && !reachesSettings ? `${notice}${PERSONAL_WORDBOOK_UNSHARED_HINT}` : notice;
  }

  /** 设置应用的 `msime_client_personal_wordbook_merge` 请求。`previous` 是上次并过的那一版，没有时为 null。 */
  static mergeRequest(stateDirectory: string, published: string, previous: string | null): string {
    const request: MergeRequest = { directory: stateDirectory, published: published };
    if (previous !== null) request.previous = previous;
    return JSON.stringify(request);
  }

  /** 合并成功了没有。成功时导入方把这一版存成「上次并过的那一版」，失败时下次再试。 */
  static mergeAccepted(response: string): boolean {
    try {
      return (JSON.parse(response) as Envelope).ok === true;
    } catch {
      return false;
    }
  }
}
