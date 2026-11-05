import { PauseRerankDiscard } from "../entry/src/main/ets/keyboard/PauseRerankDiscard";
import { NativeReplyPolicy } from "../entry/src/main/ets/keyboard/NativeReplyPolicy";

declare function require(name: "fs"): { readFileSync(path: string, encoding: "utf8"): string };
declare function require(name: "module"): { stripTypeScriptTypes(source: string): string };

// 执行真实宿主的方法体，只替换原生回复、时钟和渲染边界。
const source = require("fs").readFileSync(
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets",
  "utf8",
);
const start = source.indexOf("  private static readonly PAUSE_RERANK_MS: number = 300;");
const end = source.indexOf("  private invalidateOnlineProviders(): void {", start);
if (start < 0 || end <= start) throw new Error("找不到停顿重解方法");
const compiled = require("module").stripTypeScriptTypes(
  `class KeyboardSession {${source.slice(start, end)}}`,
);

interface PendingTimer {
  id: number;
  delay: number;
  callback: () => void;
}
class Clock {
  nextId: number = 1;
  timers: PendingTimer[] = [];
  set(callback: () => void, delay: number): number {
    const id = this.nextId++;
    this.timers.push({ id, delay, callback });
    return id;
  }
  clear(id: number): void {
    this.timers = this.timers.filter((timer: PendingTimer): boolean => timer.id !== id);
  }
  take(): PendingTimer {
    const timer = this.timers.shift();
    if (timer === undefined) throw new Error("缺少下一次停顿重排轮询");
    return timer;
  }
}
interface RerankReply {
  status: string;
  token?: number;
  moved?: boolean;
  current?: boolean;
  view?: { generation: number };
}
function create(
  startReply: () => RerankReply,
  pollReply: (handle: number, token: number, accept: boolean) => RerankReply,
) {
  const clock = new Clock();
  const shown: number[] = [];
  const calls: string[] = [];
  const Constructor = new Function(
    "client",
    "NativeReplyPolicy",
    "KeyboardLog",
    "describeError",
    "setTimeout",
    "clearTimeout",
    "PauseRerankDiscard",
    `${compiled}; return KeyboardSession;`,
  )(
    {
      startSettledRerank: (): string => JSON.stringify({ ok: true, value: startReply() }),
      pollSettledRerank: (handle: number, token: number, accept: boolean): string => {
        calls.push(`${handle}:${token}:${accept}`);
        return JSON.stringify({ ok: true, value: pollReply(handle, token, accept) });
      },
    },
    NativeReplyPolicy,
    {
      error: (text: string): never => {
        throw new Error(text);
      },
    },
    String,
    (callback: () => void, delay: number): number => clock.set(callback, delay),
    (id: number): void => clock.clear(id),
    PauseRerankDiscard,
  );
  const session = new Constructor();
  session.handle = 7;
  session.pauseRerankEpoch = 3;
  session.translationPage = null;
  session.render = (view: { generation: number }): void => {
    shown.push(view.generation);
    session.schedulePauseRerank({ editing_text: "synthetic", candidates: [{}, {}] });
  };
  return { session, clock, shown, calls };
}
function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}
const identity = create(
  (): RerankReply => ({ status: "done", moved: false, current: true, view: { generation: 12 } }),
  (): RerankReply => ({ status: "done" }),
);
identity.session.runPauseRerank(7, 3);
check(identity.shown.length === 1 && identity.shown[0] === 12, "文字未变时仍须接收新的候选身份");
identity.session.runPauseRerank(7, 2);
check(identity.shown.length === 1, "过期停顿不得发布候选身份");

let polls = 0;
const pending = create(
  (): RerankReply => ({ status: "pending", token: 101 }),
  (): RerankReply => {
    polls++;
    return polls === 1
      ? { status: "pending", token: 101 }
      : { status: "done", moved: true, current: true, view: { generation: 15 } };
  },
);
pending.session.runPauseRerank(7, 3);
const firstPoll = pending.clock.take();
check(firstPoll.delay > 0, "后台评分不能以零延时忙轮询");
firstPoll.callback();
pending.clock.take().callback();
check(pending.calls.join() === "7:101:true,7:101:true", "后台评分须携原handle、token接受结果");
check(
  pending.shown.join() === "15" && pending.clock.timers.length === 0,
  "pending完成后发布一次并停止轮询",
);

let starts = 0;
let discards = 0;
const cancelled = create(
  (): RerankReply => ({ status: "pending", token: ++starts === 1 ? 101 : 202 }),
  (_handle: number, token: number, accept: boolean): RerankReply => {
    if (!accept && token === 101) {
      return ++discards === 1 ? { status: "pending", token } : { status: "done", current: false };
    }
    return { status: "pending", token };
  },
);
cancelled.session.runPauseRerank(7, 3);
const obsolete = cancelled.clock.take();
cancelled.session.schedulePauseRerank({ editing_text: "next", candidates: [{}, {}] });
check(cancelled.calls[0] === "7:101:false", "新按键重新渲染必须丢弃旧后台结果");
const nextPause = cancelled.clock.timers.find(
  (timer: PendingTimer): boolean => timer.delay === 300,
);
if (nextPause === undefined) throw new Error("新按键须重新等待停顿");
cancelled.clock.clear(nextPause.id);
nextPause.callback();
check(starts === 1, "旧token归还前不能重复认领成新任务");
cancelled.clock.take().callback();
cancelled.clock.take().callback();
check(starts === 2 && cancelled.session.pauseRerankToken === 202, "丢弃完成后启动新token");
const currentTimer = cancelled.session.pauseRerankTimer;
obsolete.callback();
check(
  cancelled.session.pauseRerankToken === 202 && cancelled.session.pauseRerankTimer === currentTimer,
  "过期回调不得清掉新token或新定时器",
);
check(
  cancelled.shown.length === 0 && cancelled.calls.join() === "7:101:false,7:101:false",
  "取消后的旧任务须持续拒绝直到结束，不能发布",
);

cancelled.session.cancelPauseRerank();
const oldHandleCleanup = cancelled.clock.take();
cancelled.session.handle = 8;
cancelled.session.runPauseRerank(8, 5);
const newHandleTimer = cancelled.session.pauseRerankTimer;
const callsBeforeOldHandle = cancelled.calls.length;
oldHandleCleanup.callback();
check(
  cancelled.calls.length === callsBeforeOldHandle &&
    cancelled.session.pauseRerankTimer === newHandleTimer,
  "销毁重建后的旧handle回调不得轮询或修改新会话",
);
let slices = 0;
const continued = create(
  (): RerankReply => ({
    status: ++slices === 1 ? "continue" : "done",
    moved: false,
    current: true,
    view: { generation: 20 + slices },
  }),
  (): RerankReply => {
    throw new Error("同步切片不得调用后台轮询");
  },
);
continued.session.runPauseRerank(7, 3);
const nextSlice = continued.clock.take();
check(nextSlice.delay === 0, "同步切片之间须让排队的输入先走");
nextSlice.callback();
check(
  continued.shown.join() === "21,22" && continued.clock.timers.length === 0,
  "continue和done都接收文字不变的新候选身份，渲染不再启动新停顿",
);
console.log("PASS 停顿重排候选身份、后台接受与取消隔离");
