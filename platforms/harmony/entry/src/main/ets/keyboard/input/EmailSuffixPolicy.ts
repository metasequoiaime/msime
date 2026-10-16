/**
 * 邮箱后缀：光标前打到 `xxx@`（或 `xxx@` 后面接着打了域名的开头）时候选栏给出的常用后缀，移植自 Android 的 `platforms/android/java/app/msime/android/policy/EmailSuffixPolicy.java`，规则逐条相同。
 *
 * 只看光标前的文字：末尾要是 `<本地部分>@<已打的域名>`，本地部分只看紧挨 `@` 的那一个字符，必须是 ASCII 字母、数字或 `._%+-`；已打的域名可以为空，只含 ASCII 字母、数字、`.` 和 `-`。以最后一个 `@` 为准。给出以已打域名开头（不分大小写）的后缀；已打的域名已经是一个完整后缀时不再给。全角的 `＠` 不算。
 *
 * 这里不碰编辑器，也不依赖 ArkUI：宿主读光标前的文字交给这里，匹配结果只在内存里用一次，不记录、不保存。
 */

/** 常用邮箱后缀：国内常用的在前，然后是海外的。与 Android 的 `EmailSuffixPolicy.SUFFIXES` 和引擎网址模式的 `EMAIL_SUFFIXES`（`crates/engine/src/local/url.rs`）是同一份、同一个顺序，`scripts/test-email-suffixes.py` 核对几处一致。 */
export const EMAIL_SUFFIXES: string[] = [
  "@qq.com",
  "@163.com",
  "@126.com",
  "@foxmail.com",
  "@139.com",
  "@189.cn",
  "@aliyun.com",
  "@gmail.com",
  "@outlook.com",
  "@hotmail.com",
  "@live.com",
  "@icloud.com",
  "@yahoo.com",
  "@proton.me",
  "@protonmail.com",
  "@aol.com",
  "@mail.com",
  "@gmx.com",
  "@naver.com",
  "@yahoo.co.jp",
];

/** 光标前已经打到 `@` 之后的一段。 */
export interface EmailSuffixMatch {
  /** `@` 之后已经打的域名，原样，可能为空。 */
  readonly typedDomain: string;
  /** 以它开头的后缀，按 `EMAIL_SUFFIXES` 的顺序，至少一个。 */
  readonly suffixes: string[];
}

/** 选中一个后缀时对编辑器做的事：先删掉光标前 `deleteCount` 个字符（已经打的域名），再上屏 `insert`（后缀去掉 `@`，统一小写，始终是 ASCII）。 */
export interface EmailSuffixReplacement {
  readonly deleteCount: number;
  readonly insert: string;
}

export class EmailSuffixPolicy {
  /** 最多看光标前多少个字符；也是宿主向编辑器要上文时的长度，与 Android 的 `MAX_CONTEXT` 相同。 */
  static readonly MAX_CONTEXT: number = 64;

  /**
   * 这时能不能向编辑器要上文：属性已经到达（之前 `traits` 还是上一个输入框的）、不是密码框、没有组字（组字时候选栏归引擎的候选）。2in1 的候选窗只在组字时出现，后缀没有地方画，也不读。
   */
  static readsContext(
    desktop: boolean,
    attributesKnown: boolean,
    password: boolean,
    composing: boolean,
  ): boolean {
    return !desktop && attributesKnown && !password && !composing;
  }

  /** 光标前的文字末尾是不是一个打到 `@` 的邮箱；不是或没有可给的后缀时返回 null。 */
  static match(beforeCursor: string | null): EmailSuffixMatch | null {
    if (beforeCursor === null) return null;
    const end: number = beforeCursor.length;
    let at: number = end;
    while (at > 0 && EmailSuffixPolicy.domainChar(beforeCursor.charAt(at - 1))) at--;
    if (at === 0 || beforeCursor.charAt(at - 1) !== "@") return null;
    const atIndex: number = at - 1;
    if (atIndex === 0 || !EmailSuffixPolicy.localChar(beforeCursor.charAt(atIndex - 1))) {
      return null;
    }
    const typed: string = beforeCursor.substring(at, end);
    const needle: string = "@" + typed.toLowerCase();
    const suffixes: string[] = [];
    for (const suffix of EMAIL_SUFFIXES) {
      if (suffix === needle) return null;
      if (suffix.startsWith(needle)) suffixes.push(suffix);
    }
    return suffixes.length === 0 ? null : { typedDomain: typed, suffixes: suffixes };
  }

  /** 在 `match` 处补全成 `suffix`；后缀不在这次的匹配里时返回 null。 */
  static replacement(
    match: EmailSuffixMatch | null,
    suffix: string | null,
  ): EmailSuffixReplacement | null {
    if (match === null || suffix === null || match.suffixes.indexOf(suffix) < 0) return null;
    return { deleteCount: match.typedDomain.length, insert: suffix.substring(1).toLowerCase() };
  }

  /** 两次匹配给出的内容是否相同；相同时宿主不重画候选栏。 */
  static same(a: EmailSuffixMatch | null, b: EmailSuffixMatch | null): boolean {
    if (a === null || b === null) return a === b;
    return (
      a.typedDomain === b.typedDomain &&
      a.suffixes.length === b.suffixes.length &&
      a.suffixes.every((suffix: string, index: number): boolean => suffix === b.suffixes[index])
    );
  }

  private static domainChar(value: string): boolean {
    return EmailSuffixPolicy.asciiLetterOrDigit(value) || value === "." || value === "-";
  }

  private static localChar(value: string): boolean {
    return EmailSuffixPolicy.asciiLetterOrDigit(value) || "._%+-".indexOf(value) >= 0;
  }

  private static asciiLetterOrDigit(value: string): boolean {
    return (
      (value >= "a" && value <= "z") ||
      (value >= "A" && value <= "Z") ||
      (value >= "0" && value <= "9")
    );
  }
}
