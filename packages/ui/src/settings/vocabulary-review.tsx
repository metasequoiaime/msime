import { useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { readDictionaryFile } from "../dictionary/dictionary-file";
import { useMountedRef } from "./use-mounted-ref";
import { ActionButton } from "./action-button";

// The largest word list the page reads. The shared layer takes at most 8 MiB of decoded text (`vocabulary::session::MAX_IMPORT_BYTES`), well under the dictionary import's bound, so this keeps the 1 MiB the page has always read rather than following that one.
const WORDBOOK_FILE_BYTES = 1_048_576;

// Class strings live at the top of the file rather than inline in deep JSX, the way
// typing-statistics does it: the same handful of utility runs appear in a dozen places, and a
// change to the card face should be one edit rather than a dozen.
const page = "flex flex-col gap-3.5 max-phone:gap-2.5";
const heading = "m-0 text-[15px] font-semibold text-body";
const note = "mt-3.5 mb-0 text-xs leading-relaxed text-muted";
const metricRow = "flex items-baseline gap-5 max-phone:gap-4";
const metric = "flex min-w-0 flex-col gap-1";
const metricValue = "text-[30px] font-[650] leading-tight tabular-nums text-accent";
const metricLabel = "text-xs text-muted";
// The card face. It is the one surface on this page a user looks at for minutes at a time, so it
// gets the raised background and the generous minimum height rather than the ordinary section box.
const cardFace = (mobile: boolean) =>
  `flex ${mobile ? "min-h-[190px]" : "min-h-[230px]"} w-full cursor-pointer flex-col items-center justify-center gap-3 rounded-[14px] border border-edge bg-raised px-5 py-7 text-center shadow-card`;
const cardWord = (mobile: boolean) =>
  `m-0 break-anywhere font-[650] leading-tight text-body ${mobile ? "text-[30px]" : "text-[38px]"}`;
const cardPhonetic = "m-0 text-sm text-secondary";
const cardMeaning = "m-0 max-w-[46ch] text-[15px] leading-relaxed text-body";
const cardHint = "m-0 text-xs text-muted";
const answerRow = "mt-3.5 grid grid-cols-2 gap-2.5";
const answerButton =
  "min-h-[44px] rounded-[10px] border border-edge bg-card text-[15px] text-body not-disabled:hover:bg-[var(--button-secondary-hover)]";
const answerKnown =
  "min-h-[44px] rounded-[10px] border-0 bg-accent text-[15px] font-medium text-white not-disabled:hover:opacity-90";
const field = "flex items-center justify-between gap-3 py-1.5";
const empty = "mt-0.5 mb-3.5 text-center text-muted";

/** A word list the host can draw a session from. */
export type VocabularyWordbook = {
  id: string;
  name: string;
  /** How many words the book holds, for the picker's secondary line. */
  total: number;
  /** A bundled book cannot be deleted; an imported one can. */
  builtin: boolean;
  /** 来自单词本插件：在插件页卸载，这里不能删除。不是插件词书时不发这个字段。 */
  pack?: boolean;
};

/** One card, already resolved from the wordbook by the host. */
export type VocabularyCard = {
  word: string;
  /** May be empty: a user's own list often carries no transcription. */
  phonetic: string;
  meaning: string;
};

export type VocabularyReviewSettings = {
  wordbook: string;
  newPerDay: number;
  sessionLimit: number;
};

/**
 * Everything the page draws, returned whole by every mutator.
 *
 * A `Promise<void>` mutator would leave the `update()` funnel with nothing to set and push callers
 * into hand-rolled refetches, which is the double-request race the in-flight guard exists to stop.
 * The typing-statistics client states the same rule.
 */
export type VocabularyReviewStatus = {
  wordbooks: VocabularyWordbook[];
  settings: VocabularyReviewSettings;
  /** 今日待复习: cards already waiting, counted before the new-card allowance is added. */
  due: number;
  /** 已完成: answers recorded today, counting a card failed and answered again as two. */
  answeredToday: number;
  /** New cards this session will introduce. */
  introducing: number;
  /** Words in the book that are neither due nor being introduced today. */
  remaining: number;
  /** The session queue in the order it should be shown. Empty when there is nothing to review. */
  queue: VocabularyCard[];
};

export interface VocabularyReviewClient {
  load(): Promise<VocabularyReviewStatus>;
  answer(word: string, known: boolean): Promise<VocabularyReviewStatus>;
  setSettings(settings: VocabularyReviewSettings): Promise<VocabularyReviewStatus>;
  /** Absent where the host cannot hand over a file. Render nothing rather than a dead button. */
  importWordbook?(name: string, text: string): Promise<VocabularyReviewStatus>;
  /** Absent on hosts that ship only bundled books. */
  removeWordbook?(id: string): Promise<VocabularyReviewStatus>;
  reset(): Promise<VocabularyReviewStatus>;
}

/**
 * 背单词, on one of two surfaces.
 *
 * `panel` is where reviewing happens: a window the user summons, spends ten minutes in and closes.
 * `settings` is where the deck is managed — pick a book, import one, see today's numbers — and it
 * deliberately deals no cards. A settings window is a drawer people open to flip one switch and
 * leave; putting a daily study session inside it puts a destination in a utility drawer.
 */
export function VocabularyReviewPage({
  client,
  mobile = false,
  surface = "settings",
  openPanel,
}: {
  client: VocabularyReviewClient;
  mobile?: boolean;
  surface?: "settings" | "panel";
  /** Absent where the host cannot open panels; the page then keeps the review inline. */
  openPanel?: () => Promise<void>;
}) {
  const { confirm, confirmation } = useConfirm();
  const [status, setStatus] = useState<VocabularyReviewStatus>();
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [importNote, setImportNote] = useState("");
  const requestRef = useRef<Promise<VocabularyReviewStatus> | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const mounted = useMountedRef();

  useEffect(() => {
    requestRef.current = null;
    setBusy(true);
    return () => {
      requestRef.current = null;
    };
  }, [client]);

  // One funnel, one in-flight request. Two taps on 认识 in quick succession would otherwise both
  // read the same card and the second would schedule from a state the first had already replaced.
  async function update(operation: () => Promise<VocabularyReviewStatus>) {
    if (!mounted.current || requestRef.current) return;
    setBusy(true);
    setError("");
    const request = operation();
    requestRef.current = request;
    try {
      const next = await request;
      if (!mounted.current || requestRef.current !== request) return;
      setStatus(next);
    } catch {
      if (mounted.current && requestRef.current === request)
        setError("无法读取或保存背单词进度，请稍后重试。已有的进度不会被自动清空。");
    } finally {
      if (requestRef.current === request) {
        requestRef.current = null;
        if (mounted.current) setBusy(false);
      }
    }
  }

  // The client is the host binding and does not change while the page is mounted, so this reads
  // once on open. Everything after that arrives from a mutator's return value rather than a
  // refetch, which is what keeps the one in-flight request enough.
  useEffect(() => {
    void update(() => client.load());
  }, [client]);

  // With no panel to send the user to, the settings page has to keep dealing cards: a host that
  // cannot open one would otherwise offer a deck and no way to study it.
  const reviewing = surface === "panel" || !openPanel;
  const managing = surface === "settings";
  const card = status?.queue[0];
  const books = status?.wordbooks ?? [];
  const selected = status?.settings.wordbook ?? "";
  const selectedBook = books.find((book) => book.id === selected);

  async function answer(known: boolean) {
    if (!card) return;
    setRevealed(false);
    await update(() => client.answer(card.word, known));
  }

  async function chooseFile(file: File) {
    if (!client.importWordbook || requestRef.current) return;
    setImportNote("");
    setError("");
    // Real word lists arrive as UTF-16-with-BOM and GB18030 from Windows tools, which is why the
    // shared decoder exists rather than a bare File.text().
    let text: string;
    try {
      text = await readDictionaryFile(file, WORDBOOK_FILE_BYTES);
    } catch (cause) {
      // The read happens outside `update`, so nothing else would report it. Only the size refusal
      // carries a message worth showing; a failed read's own is technical and often English.
      if (mounted.current)
        setImportNote(
          file.size > WORDBOOK_FILE_BYTES && cause instanceof Error
            ? cause.message
            : "无法读取所选文件，请重新选择。",
        );
      return;
    }
    if (!mounted.current) return;
    const name = file.name.replace(/\.[^.]+$/, "").slice(0, 64) || "导入的词表";
    await update(async () => {
      const next = await client.importWordbook!(name, text);
      if (mounted.current) setImportNote(`已导入「${name}」。`);
      return next;
    });
  }

  if (!status && busy) {
    return (
      <section className={page} aria-busy="true">
        <p className={empty}>正在读取…</p>
      </section>
    );
  }

  if (!status) {
    return (
      <section className={page}>
        <p className={empty}>{error || "无法读取背单词进度。"}</p>
        <ActionButton action={() => update(() => client.load())} label="重试" />
      </section>
    );
  }

  return (
    <section className={page} aria-labelledby="vocabulary-heading">
      <div className="section">
        <h2 className={heading} id="vocabulary-heading">
          背单词
        </h2>
        <div className={metricRow} role="status" aria-live="polite">
          <div className={metric}>
            <span className={metricValue}>{status.due}</span>
            <span className={metricLabel}>今日待复习</span>
          </div>
          <div className={metric}>
            <span className={metricValue}>{status.answeredToday}</span>
            <span className={metricLabel}>已完成</span>
          </div>
          <div className={metric}>
            <span className={metricValue}>{status.remaining}</span>
            <span className={metricLabel}>未开始</span>
          </div>
        </div>
        <p className={note}>
          {reviewing
            ? "复习不会改变打字时的候选或组词。进度保存在本机，不会上传。"
            : "在这里管理词书；复习在背单词面板里进行。进度保存在本机，不会上传。"}
        </p>
        {managing && openPanel && (
          <ActionButton
            action={() => void openPanel()}
            ariaBusy={busy}
            ariaLabel="打开背单词面板"
            className="secondary"
            disabled={busy || !selected}
            label="开始复习"
          />
        )}
      </div>

      {managing && (
        <div className="section">
          <label className={field} htmlFor="vocabulary-wordbook">
            <span>词书</span>
            <select
              id="vocabulary-wordbook"
              value={selected}
              disabled={busy || books.length === 0}
              onChange={(event) =>
                void update(() =>
                  client.setSettings({ ...status.settings, wordbook: event.target.value }),
                )
              }
            >
              <option value="">未选择</option>
              {books.map((book) => (
                <option key={book.id} value={book.id}>
                  {book.name}（{book.total} 词）{book.pack ? " · 插件" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className={field} htmlFor="vocabulary-new-per-day">
            <span>每日新词</span>
            <input
              id="vocabulary-new-per-day"
              type="number"
              min={0}
              max={200}
              value={status.settings.newPerDay}
              disabled={busy}
              onChange={(event) => {
                const value = Number(event.target.value);
                if (!Number.isFinite(value) || value < 0) return;
                void update(() =>
                  client.setSettings({ ...status.settings, newPerDay: Math.trunc(value) }),
                );
              }}
            />
          </label>
          {client.importWordbook && (
            <>
              <input
                ref={fileRef}
                type="file"
                accept=".csv,.txt,text/csv,text/plain"
                hidden
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  event.target.value = "";
                  if (file) void chooseFile(file);
                }}
              />
              <ActionButton
                action={() => fileRef.current?.click()}
                ariaBusy={busy}
                ariaLabel="导入词表文件"
                className="secondary"
                disabled={busy}
                label="导入词表（CSV / TXT）"
              />
              <p className={note}>
                每行一个词，用逗号或制表符分隔：<code>单词,音标,释义</code> 或{" "}
                <code>单词,释义</code>
                。释义里有逗号时用英文引号括起来。
              </p>
            </>
          )}
          {importNote && <p className={note}>{importNote}</p>}
          {selectedBook?.pack && (
            <p className={note}>这本书来自插件，在「插件」页卸载；卸载后复习进度仍会保留。</p>
          )}
          {client.removeWordbook && selectedBook && !selectedBook.builtin && !selectedBook.pack && (
            <ActionButton
              action={async () => {
                if (
                  !(await confirm({
                    title: "删除词表",
                    message: `删除「${selectedBook.name}」及其复习进度？此操作无法撤销。`,
                    confirmLabel: "删除",
                    danger: true,
                  }))
                )
                  return;
                await update(() => client.removeWordbook!(selectedBook.id));
              }}
              ariaBusy={busy}
              className="secondary"
              disabled={busy}
              label="删除这个词表"
            />
          )}
        </div>
      )}

      {reviewing && (
        <div className="section">
          {!selected ? (
            <p className={empty}>先选一本词书。</p>
          ) : !card ? (
            <p className={empty}>今天的复习已经完成。</p>
          ) : (
            <>
              <button
                type="button"
                className={cardFace(mobile)}
                aria-label={revealed ? `${card.word} 的释义` : `显示 ${card.word} 的释义`}
                aria-pressed={revealed}
                onClick={() => setRevealed(true)}
              >
                <p className={cardWord(mobile)}>{card.word}</p>
                {card.phonetic && <p className={cardPhonetic}>{card.phonetic}</p>}
                {revealed ? (
                  <p className={cardMeaning}>{card.meaning}</p>
                ) : (
                  <p className={cardHint}>点击查看释义</p>
                )}
              </button>
              <div className={answerRow}>
                <ActionButton
                  action={() => void answer(false)}
                  ariaBusy={busy}
                  className={answerButton}
                  disabled={busy}
                  label="不认识"
                />
                <ActionButton
                  action={() => void answer(true)}
                  ariaBusy={busy}
                  className={answerKnown}
                  disabled={busy}
                  label="认识"
                />
              </div>
              <p className={note}>
                答「不认识」的词会在本次复习里再次出现；答「认识」的词按间隔安排到以后的某一天。
              </p>
            </>
          )}
        </div>
      )}

      {managing && (
        <div className="section">
          <ActionButton
            action={async () => {
              if (
                !(await confirm({
                  title: "清空进度",
                  message: "清空全部词书的复习进度？已经学过的词会从头开始，此操作无法撤销。",
                  confirmLabel: "清空",
                  danger: true,
                }))
              )
                return;
              await update(() => client.reset());
            }}
            ariaBusy={busy}
            className="secondary"
            disabled={busy}
            label="清空复习进度"
          />
        </div>
      )}

      {error && <p className={empty}>{error}</p>}
      {confirmation}
    </section>
  );
}

/** The panel surface: the card, and nothing to manage. */
export function VocabularyReviewPanel({ client }: { client: VocabularyReviewClient }) {
  return <VocabularyReviewPage client={client} surface="panel" />;
}
