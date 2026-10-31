import {
  QuickPhraseCursor,
  QuickPhraseEditor,
} from "../entry/src/main/ets/keyboard/input/QuickPhraseCursor";
import {
  PairedPunctuationCommit,
  PairedPunctuationPolicy,
} from "../entry/src/main/ets/keyboard/input/PairedPunctuationPolicy";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}

function commit(
  editor: QuickPhraseEditor,
  text: string,
  suffix: string,
  current: () => boolean,
  preview: string = "",
): boolean {
  return new QuickPhraseCursor(editor, text, suffix).submit(current, preview);
}

/** 写入只入队，读回才排空；模拟 ArkUI UI 线程与输入法线程之间的时序。 */
class Editor implements QuickPhraseEditor {
  start: number;
  end: number;
  inserts: number = 0;
  selections: number = 0;
  operations: string[] = [];
  queue: (() => void)[] = [];
  current: boolean = true;
  fail: string = "";
  interrupt: string = "";
  rewrite: boolean = false;
  refuse: boolean = false;
  ignoreSelection: boolean = false;
  truncate: boolean = false;
  autoDrain: boolean = true;

  constructor(
    public text: string,
    start: number,
    end: number = start,
  ) {
    this.start = start;
    this.end = end;
  }

  private step(operation: string): void {
    this.operations.push(operation);
    if (this.interrupt === operation) this.current = false;
    if (this.fail === operation) throw new Error("合成编辑器拒绝");
  }

  flush(): void {
    while (this.queue.length > 0) this.queue.shift()!();
  }

  insert(text: string): void {
    this.step("insert");
    this.inserts++;
    this.queue.push((): void => {
      if (this.refuse) return;
      const inserted: string = this.rewrite ? text.slice(0, -1) : text;
      this.text = this.text.slice(0, this.start) + inserted + this.text.slice(this.end);
      this.start += inserted.length;
      this.end = this.start;
    });
  }

  forward(length: number): string {
    this.step("forward");
    if (this.autoDrain) this.flush();
    return this.truncate ? "" : this.text.slice(Math.max(0, this.start - length), this.start);
  }

  backward(length: number): string {
    this.step("backward");
    if (this.autoDrain) this.flush();
    return this.text.slice(this.end, this.end + length);
  }

  cursor(): number {
    this.step("cursor");
    if (this.autoDrain) this.flush();
    return this.end;
  }

  select(position: number): void {
    this.step("select");
    this.selections++;
    this.queue.push((): void => {
      if (this.ignoreSelection) return;
      this.start = position;
      this.end = position;
    });
  }
}

let cases: number = 0;
for (const prefix of ["", "前🙂"]) {
  for (const suffix of ["尾", "😀", "e\u0301", "👩‍💻", "\n下一行", "עברית", ""]) {
    for (const replaced of ["", "选中内容"]) {
      const editor: Editor = new Editor(
        prefix + replaced + "后文",
        prefix.length,
        prefix.length + replaced.length,
      );
      const output: string = "正文" + suffix;
      const positioned: boolean = commit(editor, output, suffix, (): boolean => editor.current);
      editor.forward(1);
      check(editor.text === prefix + output + "后文", "完整提交必须一次替换原选区");
      check(editor.inserts === 1, "定位不能把正文重复上屏");
      check(editor.start === prefix.length + "正文".length, "Unicode 后缀按 UTF-16 绝对位置定位");
      check(positioned === suffix.length > 0, "空尾段无需定位");
      editor.insert("新");
      editor.forward(1);
      check(editor.text === prefix + "正文新" + suffix + "后文", "下一段必须在定位后插入");
      cases++;
    }
  }
}

const preview: Editor = new Editor("前/pair后", 1, 6);
// 预编辑的读回光标在结尾，但下一次 insert 替换整个预编辑范围。
preview.start = 6;
preview.insert = (text: string): void => {
  preview.inserts++;
  preview.queue.push((): void => {
    preview.text = "前" + text + "后";
    preview.start = 1 + text.length;
    preview.end = preview.start;
  });
};
check(
  commit(preview, "甲乙", "乙", (): boolean => true, "/pair"),
  "旧预编辑必须随完整正文替换",
);
check(preview.text === "前甲乙后" && preview.start === 2, "旧预编辑不得残留或算入目标偏移");

for (const failure of ["rewrite", "refuse", "truncate", "ignoreSelection"]) {
  const editor: Editor = new Editor("前后", 1);
  if (failure === "rewrite") editor.rewrite = true;
  if (failure === "refuse") editor.refuse = true;
  if (failure === "truncate") editor.truncate = true;
  if (failure === "ignoreSelection") editor.ignoreSelection = true;
  check(!commit(editor, "甲乙", "乙", (): boolean => true), "不可靠编辑器只交付正文");
  check(editor.inserts === 1, "降级后不得重试已交付文字");
  if (failure !== "ignoreSelection") check(editor.selections === 0, "读回失配不得移动光标");
}
for (const failure of ["forward", "backward", "cursor", "select"]) {
  const editor: Editor = new Editor("前后", 1);
  editor.fail = failure;
  check(!commit(editor, "甲乙", "乙", (): boolean => true), "读回或定位抛错保留正文");
  check(editor.inserts === 1, "异常不得重试正文");
}
for (const operation of ["forward", "backward", "insert", "cursor", "select"]) {
  const editor: Editor = new Editor("前后", 1);
  editor.interrupt = operation;
  commit(editor, "甲乙", "乙", (): boolean => editor.current);
  check(
    editor.operations[editor.operations.length - 1] === operation,
    "失焦或重入后不能继续旧派发",
  );
}
for (const prefix of ["前".repeat(QuickPhraseCursor.CONTEXT_LIMIT + 1), "前"]) {
  const editor: Editor = new Editor(prefix + "后", prefix.length);
  const suffix: string = prefix.length === 1 ? "失配" : "乙";
  check(!commit(editor, "甲乙", suffix, (): boolean => true), "超长前文或尾段失配不定位");
  check(editor.inserts === 1 && editor.selections === 0, "有界降级仍完整提交一次");
}
// 文本写入和绝对选区都可以等到布局；恢复只读回和定位，永远不重新提交。
const delayed: Editor = new Editor("前后", 1);
delayed.autoDrain = false;
const request: QuickPhraseCursor = new QuickPhraseCursor(delayed, "正文😀", "😀");
check(!request.submit((): boolean => delayed.current) && request.pending(), "旧读回必须等正文回声");
check(delayed.selections === 0 && delayed.inserts === 1, "布局前不得猜测选区");
delayed.flush();
check(
  !request.resume((): boolean => delayed.current) && request.pending(),
  "选区尚未应用时继续等回声",
);
check(delayed.selections === 1 && delayed.inserts === 1, "恢复只定位一次");
delayed.flush();
check(
  request.resume((): boolean => delayed.current) && !request.pending(),
  "目标读回确认后才能恢复预编辑",
);
check(delayed.start === 3 && delayed.text === "前正文😀后", "布局后 Unicode 目标正确");
const late: Editor = new Editor("前后", 1);
late.autoDrain = false;
const obsolete: QuickPhraseCursor = new QuickPhraseCursor(late, "正文尾", "尾");
obsolete.submit((): boolean => late.current);
late.current = false;
late.flush();
check(
  !obsolete.resume((): boolean => late.current) && late.selections === 0,
  "失焦后迟到布局不得定位",
);
check(QuickPhraseCursor.suffix("头发", "发", "頭髮", "髮") === "髮", "简繁同步转换正文和尾段");
check(
  QuickPhraseCursor.suffix("头发", "髮", "頭髮", "髮") === null,
  "转换后的巧合不能恢复失效请求",
);
check(QuickPhraseCursor.suffix("甲乙", "乙", "甲", "乙") === null, "转换后失配不得定位");
check(QuickPhraseCursor.suffix("甲乙", undefined, "甲乙", null) === null, "缺字段不产生请求");
// 打开成对补全时 `title:"${cursor}"` 以 `(` 结束：模板原样落地，不补后半个也不改回前引号，光标落在两个引号之间。会话只经 `punctuationCommit` 决定，这里测的就是会话调用的那一处。
{
  const text: string = 'title:""（';
  const suffix: string = '"（';
  check(
    PairedPunctuationPolicy.explicitCursor(text, suffix),
    "cursor phrase finished by ( asks for a position",
  );
  check(
    PairedPunctuationPolicy.explicitCursor("末尾（", ""),
    "end marker is an explicit template too",
  );
  check(PairedPunctuationPolicy.explicitCursor("好”", "”"), "a quote tail keeps the request");
  check(!PairedPunctuationPolicy.explicitCursor("（", undefined), "ordinary commit has no request");
  check(!PairedPunctuationPolicy.explicitCursor("（", null), "null suffix has no request");
  check(!PairedPunctuationPolicy.explicitCursor(text, "失配"), "mismatched suffix has no request");
  const template: PairedPunctuationCommit = PairedPunctuationPolicy.punctuationCommit(
    text,
    suffix,
    0x28,
    true,
  );
  check(
    template.commit === text && template.completion === null,
    "no closing mark after a cursor template",
  );
  const end: PairedPunctuationCommit = PairedPunctuationPolicy.punctuationCommit(
    "末尾（",
    "",
    0x28,
    true,
  );
  check(end.commit === "末尾（" && end.completion === null, "end marker lands as written too");
  const quote: PairedPunctuationCommit = PairedPunctuationPolicy.punctuationCommit(
    "好”",
    "”",
    0x22,
    true,
  );
  check(
    quote.commit === "好”" && quote.completion === null,
    "a cursor template's quote is not reopened",
  );
  const plainQuote: PairedPunctuationCommit = PairedPunctuationPolicy.punctuationCommit(
    "好”",
    undefined,
    0x22,
    true,
  );
  check(
    plainQuote.commit === "好“" && plainQuote.completion?.closing === "”",
    "without a template the quote reopens and closes",
  );
  const plain: PairedPunctuationCommit = PairedPunctuationPolicy.punctuationCommit(
    "你好（",
    null,
    0x28,
    true,
  );
  check(
    plain.commit === "你好（" && plain.completion?.closing === "）",
    "without a template ( still closes",
  );
  check(
    PairedPunctuationPolicy.punctuationCommit(text, "失配", 0x28, true).completion?.closing ===
      "）",
    "a mismatched suffix is an ordinary commit",
  );
  const off: PairedPunctuationCommit = PairedPunctuationPolicy.punctuationCommit(
    "好”",
    undefined,
    0x22,
    false,
  );
  check(off.commit === "好”" && off.completion === null, "switch off: no reopen, no closing");
  const editor: Editor = new Editor("甲丙", 1);
  check(
    commit(editor, text, suffix, (): boolean => true),
    "template commit positions the caret",
  );
  check(
    editor.text === '甲title:""（丙' && editor.start === 8 && editor.end === 8,
    "caret between the quotes, nothing appended",
  );
}
console.log(`quick phrase cursor: ${cases} 组 Unicode/选区矩阵及拒绝、重入、预编辑回归通过`);
