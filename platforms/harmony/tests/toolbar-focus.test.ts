import { ToolbarFocus, FocusToolbarPanel } from "../entry/src/main/ets/inputmethodextability/ToolbarFocus";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}
function deferred(): { promise: Promise<void>, resolve: () => void } {
  let resolve: () => void = () => {};
  const promise = new Promise<void>((done) => { resolve = done; });
  return { promise, resolve };
}

async function main(): Promise<void> {
  const events: string[] = [];
  const errors: Object[] = [];
  let current = true;
  const panel: FocusToolbarPanel = { show: async () => { events.push("show"); } };
  const focus = new ToolbarFocus((error) => errors.push(error));
  await focus.focus(panel, () => current);
  await focus.focus(panel, () => current);
  check(events.join(",") === "show,show", "每次编辑器绑定都恢复工具栏，全屏与普通窗口一致");
  current = false;
  await focus.focus(panel, () => current);
  check(events.length === 2, "旧面板不能接收显示请求");

  const release = deferred();
  const operations: string[] = [];
  const slow: FocusToolbarPanel = {
    show: async () => { operations.push("start"); await release.promise; operations.push("end"); },
  };
  const serialized = new ToolbarFocus(() => {});
  const first = serialized.focus(slow, () => true);
  await Promise.resolve();
  const obsolete = serialized.focus(panel, () => true);
  serialized.invalidate();
  const latest = serialized.focus(panel, () => true);
  release.resolve();
  await Promise.all([first, obsolete, latest]);
  check(operations.join(",") === "start,end" && events.length === 3,
    "慢显示串行完成，解绑后的排队请求失效，新焦点仍可显示");

  let failures = 0;
  const recovering = new ToolbarFocus(() => { failures++; });
  const flaky: FocusToolbarPanel = {
    show: async () => { if (failures === 0) throw new Error("synthetic panel failure"); events.push("recovered"); },
  };
  await recovering.focus(flaky, () => true);
  await recovering.focus(flaky, () => true);
  check(failures === 1 && events[events.length - 1] === "recovered", "显示失败后下次聚焦可重试");
  check(errors.length === 0, "正常场景不报告错误");
  console.log("Harmony toolbar focus visibility passed");
}
main().catch((error) => { console.error(error); throw error; });
