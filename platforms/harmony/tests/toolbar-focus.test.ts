import { ToolbarFocus, FocusToolbarPanel } from "../entry/src/main/ets/inputmethodextability/ToolbarFocus";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}
function deferred<T>(): { promise: Promise<T>, resolve: (value: T) => void } {
  let resolve: (value: T) => void = () => {};
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

async function main(): Promise<void> {
  const events: string[] = [];
  const errors: Object[] = [];
  let current = true;
  const panel: FocusToolbarPanel = {
    show: async () => { events.push("show"); },
    hide: async () => { events.push("hide"); },
  };
  const focus = new ToolbarFocus((error) => errors.push(error));
  await focus.focus(panel, () => current, async () => false);
  await focus.focus(panel, () => current, async () => false);
  check(events.join(",") === "show,show", "每次窗口聚焦均恢复非全屏工具栏，不能依赖本地可见缓存");
  await focus.focus(panel, () => current, async () => true);
  check(events.join(",") === "show,show,hide", "全屏窗口继续隐藏工具栏");
  const old = deferred<boolean>();
  const oldTask = focus.focus(panel, () => current, () => old.promise);
  await focus.focus(panel, () => current, async () => false);
  old.resolve(true);
  await oldTask;
  check(events.join(",") === "show,show,hide,show", "过期全屏查询不能隐藏新焦点的工具栏");
  const stopped = deferred<boolean>();
  const stoppedTask = focus.focus(panel, () => current, () => stopped.promise);
  focus.invalidate();
  stopped.resolve(false);
  await stoppedTask;
  check(events.length === 4, "解绑后的迟到查询不能显示工具栏");
  current = false;
  await focus.focus(panel, () => current, async () => false);
  check(events.length === 4, "旧面板不能接收显示请求");
  current = true;
  await focus.focus(panel, () => current, async () => { throw new Error("synthetic query failure"); });
  check(events.length === 5 && events[4] === "show" && errors.length === 1,
    "窗口状态查询失败时报告并尝试恢复显示");

  const release = deferred<void>();
  const operations: string[] = [];
  const slow: FocusToolbarPanel = {
    hide: async () => { operations.push("hide:start"); await release.promise; operations.push("hide:end"); },
    show: async () => { operations.push("show"); },
  };
  const serialized = new ToolbarFocus(() => {});
  const hiding = serialized.focus(slow, () => true, async () => true);
  await Promise.resolve();
  await Promise.resolve();
  const showing = serialized.focus(slow, () => true, async () => false);
  release.resolve();
  await Promise.all([hiding, showing]);
  check(operations.join(",") === "hide:start,hide:end,show", "慢隐藏完成后按新焦点显示，操作不能乱序");

  let failures = 0;
  const recovering = new ToolbarFocus(() => { failures++; });
  const flaky: FocusToolbarPanel = {
    show: async () => { if (failures === 0) throw new Error("synthetic panel failure"); events.push("recovered"); },
    hide: async () => {},
  };
  await recovering.focus(flaky, () => true, async () => false);
  await recovering.focus(flaky, () => true, async () => false);
  check(failures === 1 && events[events.length - 1] === "recovered", "一次系统显示失败后下次聚焦可重试");
  console.log("Harmony toolbar focus visibility passed");
}
main().catch((error) => { console.error(error); throw error; });
