import { useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { readDictionaryFile } from "../dictionary/dictionary-file";
import { useMountedRef } from "./use-mounted-ref";
import { useAsyncGeneration } from "./use-async-generation";
import { ActionButton } from "./action-button";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { compactPage } from "../core/page-style";
import { compactMutedNote } from "../core/text-style";
import { downloadBlob } from "../core/download-blob";
import { FluentIcon } from "../core/fluent-icons";
import { readAloudText } from "./read-aloud";

// The largest word list the page reads. The shared layer takes at most 8 MiB of decoded text (`vocabulary::session::MAX_IMPORT_BYTES`), well under the dictionary import's bound, so this keeps the 1 MiB the page has always read rather than following that one.
const WORDBOOK_FILE_BYTES = 1_048_576;

// Class strings live at the top of the file rather than inline in deep JSX, the way
// typing-statistics does it: the same handful of utility runs appear in a dozen places, and a
// change to the card face should be one edit rather than a dozen.
const page = compactPage;
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
  `m-0 wrap-anywhere font-[650] leading-tight text-body ${mobile ? "text-[30px]" : "text-[38px]"}`;
const cardPhonetic = "m-0 text-sm text-secondary";
const cardMeaning = "m-0 max-w-[46ch] text-[15px] leading-relaxed text-body";
const cardHint = compactMutedNote;
const answerRow = "mt-3.5 grid grid-cols-2 gap-2.5";
const answerButton =
  "min-h-[44px] rounded-[10px] border border-edge bg-card text-[15px] text-body not-disabled:hover:bg-[var(--button-secondary-hover)]";
const answerKnown =
  "min-h-[44px] rounded-[10px] border-0 bg-accent text-[15px] font-medium text-white not-disabled:hover:opacity-90";
const field = "flex items-center justify-between gap-3 py-1.5";
const cardAside = "mt-2.5 flex justify-center";
const speakLabel = "inline-flex items-center gap-1.5";

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
  /** 生词本：打字时从候选释义加进来的词。删除它就是清空生词本。不是生词本时不发这个字段。 */
  personal?: boolean;
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
  /** 今日待复习：今天还在等的卡片，包括今天会话要出的新卡。 */
  due: number;
  /** 已完成: answers recorded today, counting a card failed and answered again as two. */
  answeredToday: number;
  /** `due` 里的新卡数。 */
  introducing: number;
  /** 未开始：从没出过、今天也不会出的词。 */
  remaining: number;
  /** The session queue in the order it should be shown. Empty when there is nothing to review. */
  queue: VocabularyCard[];
};

export interface VocabularyReviewClient {
  load(): Promise<VocabularyReviewStatus>;
  /** 卡片与来源词书必须取自同一份已接受的状态快照。 */
  answer(wordbook: string, word: string, known: boolean): Promise<VocabularyReviewStatus>;
  setSettings(settings: VocabularyReviewSettings): Promise<VocabularyReviewStatus>;
  /** Absent where the host cannot hand over a file. Render nothing rather than a dead button. */
  importWordbook?(name: string, text: string): Promise<VocabularyReviewStatus>;
  /** Absent on hosts that ship only bundled books. */
  removeWordbook?(id: string): Promise<VocabularyReviewStatus>;
  /** 把一个词移出生词本。宿主的候选窗没有「加入生词本」时不提供，页面也就不提生词本。 */
  removePersonalWord?(word: string): Promise<VocabularyReviewStatus>;
  /** 生词本导出成的 CSV 文本，还没有生词本时是 null。 */
  exportPersonalWordbook?(): Promise<string | null>;
  /** 用系统自带的语音合成读出卡片上的英文词头（`HostCapabilities.read_aloud`）。宿主读不了时不提供，卡片上就没有喇叭。读不出来时拒绝，带一句给用户看的话。 */
  speak?(word: string): Promise<void>;
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
  saveExport,
}: {
  client: VocabularyReviewClient;
  mobile?: boolean;
  surface?: "settings" | "panel";
  /** Absent where the host cannot open panels; the page then keeps the review inline. */
  openPanel?: () => Promise<void>;
  /** 宿主把导出文件写进「下载」并回答写到了哪里（`SettingsClient.saveExport`）；没有时退回下载链接。 */
  saveExport?: (name: string, contents: string) => Promise<string | null>;
}) {
  const { confirm, confirmation } = useConfirm();
  const [status, setStatus] = useState<VocabularyReviewStatus>();
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [importNote, setImportNote] = useState("");
  const requestRef = useRef<Promise<VocabularyReviewStatus> | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const fileReadVersion = useRef(0);
  const mounted = useMountedRef();
  const generation = useAsyncGeneration(client);

  useEffect(() => {
    requestRef.current = null;
    // 旧队列不能交给新客户端作答，加载失败时也只提供重试。
    setStatus(undefined);
    setRevealed(false);
    setImportNote("");
    setBusy(true);
    return () => {
      requestRef.current = null;
    };
  }, [client]);

  // 同一代次只允许一个请求；文件读取和确认返回后，也必须在调用宿主前检查归属。
  async function update(
    operation: () => Promise<VocabularyReviewStatus>,
    current = generation.current,
  ) {
    if (!mounted.current || generation.current !== current || requestRef.current) return;
    setBusy(true);
    setError("");
    const request = operation();
    requestRef.current = request;
    try {
      const next = await request;
      if (!mounted.current || generation.current !== current || requestRef.current !== request)
        return;
      setStatus(next);
      setRevealed(false);
    } catch {
      if (mounted.current && generation.current === current && requestRef.current === request)
        setError("无法读取或保存背单词进度，请稍后重试。已有的进度不会被自动清空。");
    } finally {
      if (generation.current === current && requestRef.current === request) {
        requestRef.current = null;
        if (mounted.current) setBusy(false);
      }
    }
  }

  // 页面打开或客户端替换时读取一次；后续操作直接采纳修改返回值，避免额外刷新争用请求槽位。
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
  const personalBook = books.find((book) => book.personal);

  async function answer(known: boolean) {
    if (!card) return;
    setRevealed(false);
    await update(() => client.answer(selected, card.word, known));
  }

  async function chooseFile(file: File) {
    if (!client.importWordbook || requestRef.current) return;
    const current = generation.current;
    // 同一客户端再次选文件也会让旧读取失效，不能迟到导入或覆盖新文件的提示。
    const version = ++fileReadVersion.current;
    const isCurrent = () =>
      mounted.current && generation.current === current && fileReadVersion.current === version;
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
      if (isCurrent())
        setImportNote(
          file.size > WORDBOOK_FILE_BYTES && cause instanceof Error
            ? cause.message
            : "无法读取所选文件，请重新选择。",
        );
      return;
    }
    if (!isCurrent()) return;
    const name = file.name.replace(/\.[^.]+$/, "").slice(0, 64) || "导入的词表";
    await update(async () => {
      const next = await client.importWordbook!(name, text);
      if (isCurrent()) setImportNote(`已导入「${name}」。`);
      return next;
    }, current);
  }

  async function exportPersonal() {
    if (!client.exportPersonalWordbook || requestRef.current) return;
    const current = generation.current;
    setImportNote("");
    setError("");
    try {
      const text = await client.exportPersonalWordbook();
      if (!mounted.current || generation.current !== current) return;
      if (text === null) {
        setImportNote("生词本还是空的。");
        return;
      }
      const name = "生词本.csv";
      if (saveExport) {
        const path = await saveExport(name, text);
        if (path !== null && mounted.current && generation.current === current)
          setImportNote(`已导出到 ${path}`);
        return;
      }
      downloadBlob(new Blob([text], { type: "text/csv;charset=utf-8" }), name);
      setImportNote(`已导出「${name}」。`);
    } catch {
      if (mounted.current && generation.current === current)
        setImportNote("生词本导出失败，请稍后重试。");
    }
  }

  if (!status && busy) {
    return (
      <section className={page} aria-busy="true">
        <SettingsEmptyMessage centered>正在读取…</SettingsEmptyMessage>
      </section>
    );
  }

  if (!status) {
    return (
      <section className={page}>
        <SettingsEmptyMessage centered>{error || "无法读取背单词进度。"}</SettingsEmptyMessage>
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
          {client.removePersonalWord && (
            <p className={note}>
              {personalBook ? `生词本里有 ${personalBook.total} 个词。` : "生词本还是空的。"}
              打字时在候选的操作菜单里选「加入生词本」，候选旁显示的释义就会收进生词本；它只存在这台设备上。
            </p>
          )}
          {client.exportPersonalWordbook && personalBook && (
            <ActionButton
              action={() => void exportPersonal()}
              ariaBusy={busy}
              className="secondary"
              disabled={busy}
              label="导出生词本"
            />
          )}
          {client.removeWordbook && selectedBook && !selectedBook.builtin && !selectedBook.pack && (
            <ActionButton
              action={async () => {
                const current = generation.current;
                if (
                  !(await confirm(
                    selectedBook.personal
                      ? {
                          title: "清空生词本",
                          message: `清空生词本里的 ${selectedBook.total} 个词及其复习进度？此操作无法撤销。`,
                          confirmLabel: "清空",
                          danger: true,
                        }
                      : {
                          title: "删除词表",
                          message: `删除「${selectedBook.name}」及其复习进度？此操作无法撤销。`,
                          confirmLabel: "删除",
                          danger: true,
                        },
                  ))
                )
                  return;
                await update(() => client.removeWordbook!(selectedBook.id), current);
              }}
              ariaBusy={busy}
              className="secondary"
              disabled={busy}
              label={selectedBook.personal ? "清空生词本" : "删除这个词表"}
            />
          )}
        </div>
      )}

      {reviewing && (
        <div className="section">
          {!selected ? (
            <SettingsEmptyMessage centered>先选一本词书。</SettingsEmptyMessage>
          ) : !card ? (
            <SettingsEmptyMessage centered>今天的复习已经完成。</SettingsEmptyMessage>
          ) : (
            <>
              <ActionButton
                action={() => setRevealed(true)}
                ariaLabel={revealed ? `${card.word} 的释义` : `显示 ${card.word} 的释义`}
                ariaPressed={revealed}
                className={cardFace(mobile)}
                label={
                  <>
                    <p className={cardWord(mobile)}>{card.word}</p>
                    {card.phonetic && <p className={cardPhonetic}>{card.phonetic}</p>}
                    {revealed ? (
                      <p className={cardMeaning}>{card.meaning}</p>
                    ) : (
                      <p className={cardHint}>点击查看释义</p>
                    )}
                  </>
                }
              />
              {client.speak && readAloudText(card.word) && (
                <div className={cardAside}>
                  <ActionButton
                    action={async () => {
                      setError("");
                      try {
                        await client.speak!(readAloudText(card.word)!);
                      } catch (reason) {
                        setError(reason instanceof Error ? reason.message : String(reason));
                      }
                    }}
                    ariaLabel={`朗读 ${card.word}`}
                    className="secondary"
                    label={
                      <span className={speakLabel}>
                        <FluentIcon name="speaker_2" size={16} />
                        朗读
                      </span>
                    }
                  />
                </div>
              )}
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
              {client.removePersonalWord && selectedBook?.personal && (
                <div className={cardAside}>
                  <ActionButton
                    action={() => void update(() => client.removePersonalWord!(card.word))}
                    ariaBusy={busy}
                    ariaLabel={`把 ${card.word} 移出生词本`}
                    className="secondary"
                    disabled={busy}
                    label="移出生词本"
                  />
                </div>
              )}
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
              const current = generation.current;
              if (
                !(await confirm({
                  title: "清空进度",
                  message: "清空全部词书的复习进度？已经学过的词会从头开始，此操作无法撤销。",
                  confirmLabel: "清空",
                  danger: true,
                }))
              )
                return;
              await update(() => client.reset(), current);
            }}
            ariaBusy={busy}
            className="secondary"
            disabled={busy}
            label="清空复习进度"
          />
        </div>
      )}

      {error && <SettingsEmptyMessage centered>{error}</SettingsEmptyMessage>}
      {confirmation}
    </section>
  );
}

/** The panel surface: the card, and nothing to manage. */
export function VocabularyReviewPanel({ client }: { client: VocabularyReviewClient }) {
  return <VocabularyReviewPage client={client} surface="panel" />;
}
