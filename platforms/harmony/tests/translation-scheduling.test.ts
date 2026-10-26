import { NativeReplyPolicy } from "../entry/src/main/ets/keyboard/NativeReplyPolicy";
import {
  TranslationEntry,
  TranslationPolicy,
  TranslationQuery,
} from "../entry/src/main/ets/keyboard/candidate/TranslationPolicy";

declare function require(name: "fs"): { readFileSync(path: string, encoding: "utf8"): string };
declare function require(name: "module"): { stripTypeScriptTypes(source: string): string };

interface Session {
  handle: number;
  translationSignature: string;
  translationEpoch: number;
  translationTimer: number;
  scheduleTranslations(): void;
  invalidateTranslationProviders(): void;
  translationCurrent(handle: number, epoch: number): boolean;
  appendOfflineGloss(query: TranslationQuery, entries: TranslationEntry[]): void;
  offlineTargetGlosses(query: TranslationQuery): Map<string, TranslationEntry[]>;
  render(view: { entries: TranslationEntry[] }): void;
  fetchTargetTranslations(
    query: TranslationQuery,
    target: string,
    provider: string,
    entries: TranslationEntry[],
    answered: TranslationEntry[],
    epoch: number,
    handle: number,
  ): Promise<{ current: boolean; providerComplete: boolean }>;
}

function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

// 直接执行宿主的方法体，避免测试里复制一份调度算法；原生边界、渲染和时钟使用合成替身。
const source = require("fs").readFileSync(
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets",
  "utf8",
);
const start = source.indexOf("  private scheduleTranslations(): void {");
const end = source.indexOf("  private async fetchTargetTranslations(", start);
check(start >= 0 && end > start, "没有找到宿主的翻译调度方法");
const compiled = require("module").stripTypeScriptTypes(
  `class Session {${source.slice(start, end)}\n}`,
);

function fixture(online: boolean): {
  session: Session;
  timers: { callback: () => void; delay: number }[];
  shown: TranslationEntry[][];
  reads(): number;
  calls(): number;
  next(): void;
  release(): void;
} {
  let generation = 1;
  let readCount = 0;
  let callCount = 0;
  let release: () => void = (): void => {};
  const timers: { callback: () => void; delay: number }[] = [];
  const shown: TranslationEntry[][] = [];
  const query = (): TranslationQuery => ({
    generation,
    target_language: "en",
    english_gloss: true,
    resources: "/synthetic",
    candidates: [{ text: `合成词${generation}`, source: 0 }],
    custom_translation: online
      ? { enabled: true, endpoint: "https://example.invalid", api_key: "synthetic" }
      : null,
  });
  const client = {
    translationQuery: (): string => JSON.stringify({ ok: true, value: query() }),
    applyTranslations: (_handle: number, requested: number, text: string): string =>
      JSON.stringify({
        ok: true,
        value: { applied: requested === generation, view: { entries: JSON.parse(text) } },
      }),
  };
  const Constructor = new Function(
    "client",
    "NativeReplyPolicy",
    "KeyboardFormFactor",
    "TranslationPolicy",
    "KeyboardLog",
    "describeError",
    "setTimeout",
    `${compiled}; return Session;`,
  )(
    client,
    NativeReplyPolicy,
    { isDesktop: (): boolean => false },
    TranslationPolicy,
    {
      error: (message: string): never => {
        throw new Error(message);
      },
    },
    String,
    (callback: () => void, delay: number): number => timers.push({ callback, delay }),
  ) as new () => Session;
  const session = new Constructor();
  session.handle = 7;
  session.translationEpoch = 0;
  session.translationSignature = "";
  session.translationTimer = -1;
  session.invalidateTranslationProviders = (): void => {
    session.translationEpoch++;
    session.translationSignature = "";
    timers.length = 0;
  };
  session.translationCurrent = (handle: number, epoch: number): boolean =>
    handle === session.handle && epoch === session.translationEpoch;
  session.appendOfflineGloss = (request: TranslationQuery, entries: TranslationEntry[]): void => {
    readCount++;
    entries.push({ text: request.candidates[0].text, translation: `local-${request.generation}` });
  };
  session.offlineTargetGlosses = (): Map<string, TranslationEntry[]> => new Map();
  session.render = (view): void => {
    shown.push(view.entries);
    session.scheduleTranslations();
  };
  session.fetchTargetTranslations = async (
    _query,
    _target,
    _provider,
    _entries,
    _answered,
    epoch,
    handle,
  ) => {
    callCount++;
    await new Promise<void>((resolve): void => {
      release = resolve;
    });
    return { current: session.translationCurrent(handle, epoch), providerComplete: true };
  };
  return {
    session,
    timers,
    shown,
    reads: (): number => readCount,
    calls: (): number => callCount,
    next: (): void => {
      generation++;
      session.scheduleTranslations();
    },
    release: (): void => release(),
  };
}

// 方法体按名字取出：从声明行到第一个两格缩进的右花括号。
function method(name: string): string {
  let begin = source.indexOf(`  private ${name}(`);
  if (begin < 0) begin = source.indexOf(`  private async ${name}(`);
  check(begin >= 0, `没有找到宿主的 ${name}`);
  const close = source.indexOf("\n  }\n", begin);
  check(close > begin, `${name} 没有结束`);
  return source.slice(begin, close + 4);
}

interface ScopedSession extends Session {
  translationTimer: number;
  translationCache: Map<string, string>;
  translationNegativeCache: Map<string, number>;
  fetchCustomTranslation(
    config: unknown,
    item: { text: string },
  ): Promise<{ value: string | null; completed: boolean }>;
}

// 「仅当前高亮」：共享查询的 candidates 只有高亮的那一个，另带一份覆盖整页、不带任何服务的 gloss_query（代次相同）。高亮移动推进代次。这里跑真实的调度、在线阶段、缓存与记录方法，原生边界、离线词典和网络用合成替身。
function scopedFixture(): {
  session: ScopedSession;
  timers: { callback: () => void; delay: number }[];
  applied: TranslationEntry[][];
  attempts: number;
  asked: string[];
  offlineReads: string[][];
  move(to: number): void;
  reply(): void;
} {
  const page = ["甲", "乙", "丙", "丁"];
  // 离线英文词典只认得「丁」，其余候选的英文只能从在线服务来。
  const dictionary = new Map<string, string>([["丁", "dict-丁"]]);
  let highlighted = 0;
  let generation = 1;
  const timers: { callback: () => void; delay: number }[] = [];
  const applied: TranslationEntry[][] = [];
  const asked: string[] = [];
  const offlineReads: string[][] = [];
  const replies: (() => void)[] = [];
  const state = { attempts: 0 };
  const documentFor = (texts: string[], services: boolean): TranslationQuery => ({
    generation,
    target_language: "en",
    target_languages: ["en"],
    english_gloss: true,
    resources: "/synthetic",
    user_data: "/synthetic-user",
    candidates: texts.map((text: string) => ({ text, source: 0 })),
    provider: services ? "custom" : "none",
    translation_account: false,
    custom_translation: services
      ? { enabled: true, endpoint: "https://example.invalid", api_key: "synthetic" }
      : null,
    tencent_tmt: null,
    niutrans: null,
    ai_assistant: null,
  });
  const client = {
    translationQuery: (): string => {
      const query: TranslationQuery = documentFor([page[highlighted]], true);
      query.gloss_query = documentFor(page, false);
      return JSON.stringify({ ok: true, value: query });
    },
    applyTranslations: (_handle: number, requested: number, text: string): string => {
      state.attempts++;
      const current = requested === generation;
      if (current) applied.push(JSON.parse(text) as TranslationEntry[]);
      return JSON.stringify({ ok: true, value: { applied: current, view: {} } });
    },
    translationPlan: (document: string): string => {
      const request = JSON.parse(document) as {
        target_language: string;
        candidates: { text: string }[];
      };
      return JSON.stringify({
        ok: true,
        value: request.candidates.map((candidate: { text: string }) => ({
          text: candidate.text,
          key: candidate.text,
          source_language: "zh",
          target_language: request.target_language,
        })),
      });
    },
    translationGlossSave: (): string => JSON.stringify({ ok: true, value: null }),
  };
  const body = `${source.slice(start, end)}\n${method("fetchTargetTranslations")}\n${method("recordTranslation")}`;
  const Constructor = new Function(
    "client",
    "NativeReplyPolicy",
    "KeyboardFormFactor",
    "TranslationPolicy",
    "KeyboardLog",
    "describeError",
    "setTimeout",
    `${require("module").stripTypeScriptTypes(`class Session {${body}\n}`)}; return Session;`,
  )(
    client,
    NativeReplyPolicy,
    { isDesktop: (): boolean => false },
    TranslationPolicy,
    {
      error: (message: string): never => {
        throw new Error(message);
      },
    },
    String,
    (callback: () => void, delay: number): number => timers.push({ callback, delay }),
  ) as new () => ScopedSession;
  const session = new Constructor();
  session.handle = 7;
  session.translationEpoch = 0;
  session.translationSignature = "";
  session.translationTimer = -1;
  session.translationCache = new Map<string, string>();
  session.translationNegativeCache = new Map<string, number>();
  session.invalidateTranslationProviders = (): void => {
    session.translationEpoch++;
    session.translationSignature = "";
    timers.length = 0;
  };
  session.translationCurrent = (handle: number, epoch: number): boolean =>
    handle === session.handle && epoch === session.translationEpoch;
  session.appendOfflineGloss = (request: TranslationQuery, entries: TranslationEntry[]): void => {
    offlineReads.push(request.candidates.map((candidate) => candidate.text));
    for (const candidate of request.candidates) {
      const gloss = dictionary.get(candidate.text);
      if (gloss !== undefined) entries.push({ text: candidate.text, translation: gloss });
    }
  };
  session.offlineTargetGlosses = (): Map<string, TranslationEntry[]> => new Map();
  // 和宿主一样：应用回答后重绘，重绘又会问一次翻译查询。
  session.render = (): void => session.scheduleTranslations();
  session.fetchCustomTranslation = async (_config, item) => {
    asked.push(item.text);
    await new Promise<void>((resolve): void => {
      replies.push(resolve);
    });
    return { value: `net-${item.text}`, completed: true };
  };
  return {
    session,
    timers,
    applied,
    get attempts(): number {
      return state.attempts;
    },
    asked,
    offlineReads,
    move: (to: number): void => {
      highlighted = to;
      generation++;
      session.scheduleTranslations();
    },
    reply: (): void => {
      for (const resolve of replies.splice(0)) resolve();
    },
  };
}

function settle(): Promise<void> {
  return new Promise<void>((resolve): void => {
    setTimeout(resolve, 0);
  });
}

async function scoped(): Promise<void> {
  const scope = scopedFixture();
  scope.session.scheduleTranslations();
  check(
    scope.offlineReads.some((texts: string[]) => texts.join() === "甲,乙,丙,丁"),
    "gloss_query 要用处理普通查询的同一条离线路径算整页的释义",
  );
  const first = scope.applied[0] ?? [];
  check(
    first.some(
      (entry) => entry.text === "丁" && entry.translation === "dict-丁" && entry.gloss === true,
    ),
    "gloss_query 的回答要标 gloss: true 交给 applyTranslations",
  );
  check(
    first.every((entry) => entry.text === "甲" || entry.gloss === true),
    "非高亮候选只能拿到标了 gloss 的离线释义",
  );
  check(scope.timers.length === 1, "gloss_query 不带服务，不为它另起网络定时器");
  scope.timers[0].callback();
  await settle();
  check(scope.asked.join() === "甲", "在线服务只为高亮的候选请求");
  scope.reply();
  await settle();
  check(
    scope.applied.some((entries) =>
      entries.some(
        (entry) => entry.text === "甲" && entry.translation === "net-甲" && entry.gloss !== true,
      ),
    ),
    "高亮候选的在线翻译不标 gloss",
  );

  // 高亮快速移过「乙」停在「丙」：途经的候选不发请求，旧定时器作废。
  scope.move(1);
  scope.move(2);
  check(scope.timers.length === 1, "高亮移动后只留最新的一个定时器");
  scope.timers[0].callback();
  await settle();
  check(scope.asked.join() === "甲,丙", "高亮快速移过的候选不发请求");
  // 「丙」的回答还没到，高亮又回到「甲」：旧请求的回答要丢弃，「甲」复用缓存。
  const attempts = scope.attempts;
  scope.move(0);
  scope.reply();
  await settle();
  check(
    scope.applied.every((entries) => entries.every((entry) => entry.translation !== "net-丙")),
    "高亮移走后，旧请求的回答不能应用",
  );
  check(scope.attempts > attempts, "移回来的新查询照常应用离线释义");
  scope.timers[0].callback();
  await settle();
  check(scope.asked.join() === "甲,丙", "移回来的候选复用缓存，不再请求");
  const last = scope.applied[scope.applied.length - 1] ?? [];
  check(
    last.some(
      (entry) => entry.text === "甲" && entry.translation === "net-甲" && entry.gloss !== true,
    ),
    "缓存的翻译照旧作为候选词翻译交回",
  );

  // 高亮停在离线词典认得的「丁」：主查询的回答（候选词翻译）不标 gloss，gloss_query 的同一条释义标 gloss。
  scope.move(3);
  const own = scope.applied[scope.applied.length - 1] ?? [];
  check(
    own.some((entry) => entry.text === "丁" && entry.gloss !== true) &&
      own.some((entry) => entry.text === "丁" && entry.gloss === true),
    "主查询与 gloss_query 的回答一起交给 applyTranslations，只有后者标 gloss",
  );
}

async function main(): Promise<void> {
  const offline = fixture(false);
  offline.session.scheduleTranslations();
  check(offline.shown[0]?.[0]?.translation === "local-1", "离线结果必须在调度方法返回之前显示");
  check(offline.timers.length === 0, "没有在线服务时不启动网络定时器");
  offline.session.scheduleTranslations();
  check(offline.reads() === 1, "重绘和相同候选页不得重复查词");

  const online = fixture(true);
  online.session.scheduleTranslations();
  check(online.shown[0]?.[0]?.translation === "local-1", "在线服务不能拖延本地结果");
  check(online.calls() === 0 && online.timers[0]?.delay === 500, "在线服务仍等待停键 500 毫秒");
  online.timers[0].callback();
  check(online.calls() === 1 && online.reads() === 1, "网络阶段复用已查询的离线结果");
  online.next();
  online.release();
  await Promise.resolve();
  await Promise.resolve();
  check(
    online.shown.length === 2 && online.shown[1][0].translation === "local-2",
    "过时网络回复不能覆盖新候选页",
  );
  await scoped();
  console.log("candidate translation scheduling: 20 checks passed");
}

void main();
