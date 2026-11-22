import { ToolbarFocus, FocusToolbarPanel } from "../entry/src/main/ets/inputmethodextability/ToolbarFocus";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}
function deferred<T>(): { promise: Promise<T>, resolve: (value: T) => void } {
  let resolve: (value: T) => void = () => {};
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

// 模拟系统状态面板，只记录显示请求。`ToolbarFocus` 不知道调用窗口是否全屏；全屏时也显示、不查询窗口状态是 `followCallingWindow` 的接线，由 toolbar-setup-adapter.test.cjs 执行真实方法覆盖。
class FakePanel implements FocusToolbarPanel {
  readonly events: string[] = [];
  async show(): Promise<void> { this.events.push("show"); }
}

async function main(): Promise<void> {
  const errors: Object[] = [];
  let current = true;
  const panel = new FakePanel();
  const focus = new ToolbarFocus((error) => errors.push(error));
  await focus.focus(panel, () => current);
  await focus.focus(panel, () => current);
  check(panel.events.join(",") === "show,show", "每次绑定都请求显示，不能依赖本地可见缓存");

  current = false;
  await focus.focus(panel, () => current);
  check(panel.events.length === 2, "旧面板不能接收显示请求");
  current = true;

  // 前一个显示还没返回时，排在后面的请求按顺序执行；轮到之前已被取代或废止的请求直接丢弃。
  const release = deferred<void>();
  const operations: string[] = [];
  let calls = 0;
  const slow: FocusToolbarPanel = {
    show: async () => {
      const call = ++calls;
      operations.push(`show${call}:start`);
      if (call === 1) await release.promise;
      operations.push(`show${call}:end`);
    },
  };
  const serialized = new ToolbarFocus(() => {});
  const first = serialized.focus(slow, () => true);
  await Promise.resolve();
  await Promise.resolve();
  const superseded = serialized.focus(slow, () => true);
  const latest = serialized.focus(slow, () => true);
  release.resolve();
  await Promise.all([first, superseded, latest]);
  check(operations.join(",") === "show1:start,show1:end,show2:start,show2:end",
    "慢显示完成后才执行下一个，被新焦点取代的排队请求丢弃，操作不能乱序");

  const blocked = deferred<void>();
  let shown = 0;
  const gate: FocusToolbarPanel = { show: async () => { shown++; if (shown === 1) await blocked.promise; } };
  const stopping = new ToolbarFocus(() => {});
  const running = stopping.focus(gate, () => true);
  await Promise.resolve();
  await Promise.resolve();
  const queued = stopping.focus(gate, () => true);
  stopping.invalidate();
  blocked.resolve();
  await Promise.all([running, queued]);
  check(shown === 1, "解绑后排队中的显示请求不能再执行");

  let failures = 0;
  const flakyEvents: string[] = [];
  const recovering = new ToolbarFocus(() => { failures++; });
  const flaky: FocusToolbarPanel = {
    show: async () => { if (failures === 0) throw new Error("synthetic panel failure"); flakyEvents.push("recovered"); },
  };
  await recovering.focus(flaky, () => true);
  await recovering.focus(flaky, () => true);
  check(failures === 1 && flakyEvents.join(",") === "recovered", "一次系统显示失败后下次绑定可重试");

  const placed: string[] = [];
  const opening: FocusToolbarPanel = { show: async () => { placed.push("show"); } };
  const other: FocusToolbarPanel = { show: async () => { placed.push("other"); } };
  const setup = new ToolbarFocus(() => {});
  setup.beginSetup(opening);
  await setup.focus(opening, () => true);
  check(placed.length === 0, "面板尚未调整尺寸和位置时，焦点不能抢先显示它");
  const openingRelease = deferred<void>();
  const busy: FocusToolbarPanel = { show: async () => { await openingRelease.promise; } };
  const pending = setup.focus(busy, () => true);
  const queuedOpening = setup.focus(opening, () => true);
  setup.endSetup(other);
  openingRelease.resolve();
  await Promise.all([pending, queuedOpening]);
  check(placed.length === 0, "其他面板结束准备不能放行仍在准备的面板");
  setup.endSetup(opening);
  await setup.focus(opening, () => true);
  check(placed.join(",") === "show", "准备完成后的焦点照常显示");
  check(errors.length === 0, "正常路径不报告错误");
  console.log("Harmony toolbar focus visibility passed");
}
main().catch((error) => { console.error(error); throw error; });
