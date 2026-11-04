export {};

declare function require(name: "fs"): { readFileSync(path: string, encoding: "utf8"): string };
declare function require(name: "module"): { stripTypeScriptTypes(source: string): string };

interface VoiceView {
  voiceText: string;
  voiceNotice: string;
  voiceDone: string;
  voiceDoneTimer: number;
  surface: number;
  touchDesign(): boolean;
  show(surface: number): void;
  deliverVoice(text: string | null): void;
}

function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

// 执行真实的结果交付方法，只替换原生会话与时钟，避免在测试里另写一份收尾逻辑。
const source = require("fs").readFileSync(
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets",
  "utf8",
);
const start = source.indexOf("  private deliverVoice(");
const end = source.indexOf("  private clearVoiceDone(", start);
check(start >= 0 && end > start, "没有找到语音结果交付方法");
const compiled = require("module").stripTypeScriptTypes(
  `class VoiceView {${source.slice(start, end)}\n}`,
);

function fixture(output: string): {
  view: VoiceView;
  inserted: string[];
  timers: (() => void)[];
} {
  const timers: (() => void)[] = [];
  const inserted: string[] = [];
  const Constructor = new Function(
    "KeyboardSession",
    "VoiceRecognitionPolicy",
    "SURFACE_VOICE",
    "SURFACE_NONE",
    "VOICE_DONE_NOTICE_MS",
    "setTimeout",
    `${compiled}; return VoiceView;`,
  )(
    {
      shared: {
        voiceTargetMatches: (): boolean => true,
        voicePunctuation: (): string => output,
        markVoicePunctuationApplied(): void {},
        insertVoiceResult(text: string): void {
          inserted.push(text);
        },
      },
    },
    { skipsReview: (): boolean => true },
    1,
    0,
    1000,
    (callback: () => void): number => {
      timers.push(callback);
      return timers.length;
    },
  ) as new () => VoiceView;
  const view = new Constructor();
  Object.assign(view, {
    voiceText: "合成识别文本。",
    voiceNotice: "",
    voiceDone: "",
    voiceDoneTimer: -1,
    surface: 1,
    touchDesign: (): boolean => true,
    show(surface: number): void {
      this.surface = surface;
    },
  });
  return { view, inserted, timers };
}

// 共享层的标点算法另有单测；这里覆盖它确实改动文本后的视图生命周期。
for (const output of ["合成识别文本。", "合成识别文本", "合成 识别文本"]) {
  const run = fixture(output);
  run.view.deliverVoice("合成识别文本。");
  check(run.inserted.length === 1 && run.inserted[0] === output, "只上屏处理后的文本一次");
  check(run.view.voiceDone === output, "成功提示显示处理后的文本");
  check(run.timers.length === 1, "成功结果安排一次收尾");
  run.timers[0]();
  check(run.view.voiceDone === "", "标点改变文本后成功提示仍应清除");
  check(run.view.surface === 0, "标点改变文本后语音面板仍应自动收起");
}

const newer = fixture("合成识别文本");
newer.view.deliverVoice("合成识别文本。");
newer.view.voiceDone = "另一条合成结果";
newer.timers[0]();
check(newer.view.voiceDone === "另一条合成结果", "过期回调不能清掉后来结果的提示");
check(newer.view.surface === 1, "过期回调不能收起后来的语音面板");

const empty = fixture("");
empty.view.deliverVoice("。");
check(
  empty.inserted.length === 0 && empty.timers.length === 0,
  "仅标点的结果不上屏、不安排成功收尾",
);
check(empty.view.voiceNotice === "没有识别到文字。", "仅标点的结果给出空内容提示");
console.log(
  "voice result lifecycle: changed punctuation closes the panel, stale results stay intact",
);
