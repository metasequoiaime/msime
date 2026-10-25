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
  console.log("candidate translation scheduling: 7 checks passed");
}

void main();
