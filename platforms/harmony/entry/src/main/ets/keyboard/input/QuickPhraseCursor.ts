/** 只封装编辑器交付，不解析模板；正文和后缀来自共享运行时。 */
export interface QuickPhraseEditor {
  insert(text: string): void;
  forward(length: number): string;
  backward(length: number): string;
  cursor(): number;
  select(position: number): void;
}

interface PhraseContext {
  left: string;
  right: string;
}

export class QuickPhraseCursor {
  /** 为定位读回的前后文各自有界；长文或截短上下文仍完整上屏，只放弃定位。 */
  static readonly CONTEXT_LIMIT: number = 4096;
  private before: PhraseContext | null = null;
  private selected: boolean = false;
  private finished: boolean = false;

  constructor(
    private editor: QuickPhraseEditor,
    private text: string,
    private tail: string,
  ) {}

  /** 转换前后都必须匹配，不能让转换后的巧合修复原本失配的尾段。 */
  static suffix(
    text: string,
    suffix: string | null | undefined,
    output: string,
    converted: string | null,
  ): string | null {
    return typeof suffix === "string" &&
      text.endsWith(suffix) &&
      converted !== null &&
      output.endsWith(converted)
      ? converted
      : null;
  }

  pending(): boolean {
    return !this.finished;
  }

  /** 完整提交一次。读回可能早于布局；尚未看到正文时等自己的选区回声，绝不重发正文。 */
  submit(current: () => boolean, preview: string = ""): boolean {
    this.before = this.context(current);
    if (this.before !== null && preview.length > 0) {
      if (this.before.left.endsWith(preview)) {
        this.before.left = this.before.left.slice(0, this.before.left.length - preview.length);
      } else {
        this.before = null;
      }
    }
    if (!current()) {
      this.finished = true;
      return false;
    }
    this.editor.insert(this.text);
    if (
      !current() ||
      this.before === null ||
      this.tail.length === 0 ||
      !this.text.endsWith(this.tail)
    ) {
      this.finished = true;
      return false;
    }
    return this.resume(current);
  }

  /** 仅由当前编辑器自有改动的折叠选区回声恢复；调用者负责失焦、外部编辑和超时作废。 */
  resume(current: () => boolean): boolean {
    if (this.finished || !current() || this.before === null) return false;
    const context: PhraseContext | null = this.context(current);
    if (context === null) {
      this.finished = true;
      return false;
    }
    const left: string = this.before.left + this.text;
    const target: number = left.length - this.tail.length;
    if (!this.selected) {
      if (context.left !== left || context.right !== this.before.right) return false;
      // 完整左文长度必须等于绝对光标位置，拒绝截短上下文和未折叠的正向选区。
      if (this.index(current) !== left.length || !current()) return false;
      this.selected = true;
      try {
        this.editor.select(target);
      } catch (_) {
        this.finished = true;
        return false;
      }
      if (!current()) return false;
      return this.resume(current);
    }
    if (
      context.left !== left.slice(0, target) ||
      context.right !== this.tail + this.before.right ||
      this.index(current) !== target
    )
      return false;
    this.finished = true;
    return true;
  }

  private context(current: () => boolean): PhraseContext | null {
    try {
      if (!current()) return null;
      const left: string = this.editor.forward(QuickPhraseCursor.CONTEXT_LIMIT + 1);
      if (!current()) return null;
      const right: string = this.editor.backward(QuickPhraseCursor.CONTEXT_LIMIT + 1);
      if (
        !current() ||
        left.length > QuickPhraseCursor.CONTEXT_LIMIT ||
        right.length > QuickPhraseCursor.CONTEXT_LIMIT
      )
        return null;
      return { left, right };
    } catch (_) {
      return null;
    }
  }

  private index(current: () => boolean): number | null {
    try {
      if (!current()) return null;
      const index: number = this.editor.cursor();
      return current() && Number.isSafeInteger(index) && index >= 0 ? index : null;
    } catch (_) {
      return null;
    }
  }
}
