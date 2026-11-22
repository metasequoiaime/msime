// 执行 KeyboardExtensionAbility.ets 里真实的悬浮工具栏方法（建面板、偏好重载、绑定与解绑编辑器、服务创建期间的待绑定编辑器），系统面板换成记录调用的替身，覆盖 ToolbarFocus 单测碰不到的接线。
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const abilityPath = path.join(root, "platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets");
const compiled = path.join(root, "target/harmony-tests/entry/src/main/ets");
const staging = path.join(root, "target/harmony-tests/toolbar-setup-adapter");
fs.rmSync(staging, { recursive: true, force: true });
fs.mkdirSync(staging, { recursive: true });
const { ToolbarFocus } = require(path.join(compiled, "inputmethodextability/ToolbarFocus.js"));
const source = fs.readFileSync(abilityPath, "utf8");

function extract(pattern, name) {
  const match = source.match(pattern);
  assert.ok(match, `找不到生产代码 ${name}`);
  return match[0];
}
function method(name) {
  return extract(new RegExp(`^  (?:private )?(?:async )?${name}\\([^]*?^  }\\n`, "m"), name);
}
// 箭头函数字段：`started` 的函数体缩进 4 格，`stopped` 缩进 2 格。
const started = extract(/^  private readonly started:[^]*?^    };\n/m, "started");
const stopped = extract(/^  private readonly stopped:[^]*?^  };\n/m, "stopped");
// onCreate 里面板建好后接上服务创建期间获得焦点的编辑器的那一段。
const pendingBlock = extract(/^        if \(this\.pending !== undefined\) \{\n          KeyboardSession\.shared\.attach\(this\.pending\);[^]*?^        }\n/m, "onCreate pending");
const margin = extract(/^const TOOLBAR_MARGIN_VP\b[^\n]*\n/m, "TOOLBAR_MARGIN_VP");

const program = `${margin}
export class Host {
${method("resizeToolbar")}
${method("reconcileStatusPanels")}
${method("openToolbar")}
${method("closeToolbar")}
${method("followCallingWindow")}
${started}
${stopped}
  attachPending(desktop: boolean): void {
${pendingBlock}
  }
}
`;
const input = path.join(staging, "production-methods.ts");
fs.writeFileSync(input, program);
execFileSync(
  path.join(root, "apps/desktop/node_modules/.bin/tsc"),
  ["--ignoreConfig", "--noCheck", "--target", "es2021", "--module", "commonjs", "--outDir", path.join(staging, "compiled"), input],
  { stdio: "inherit" },
);

function deferred() {
  let resolve = () => {};
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}
const tick = () => new Promise((resolve) => setImmediate(resolve));
async function until(condition, message) {
  for (let i = 0; i < 200; i++) {
    if (condition()) return;
    await tick();
  }
  assert.fail(message);
}

// 系统状态面板替身：调用窗口一律报告全屏，宿主不应再查询它或因此隐藏工具栏。
class FakeToolbar {
  constructor() {
    this.events = [];
    this.resizeGate = undefined;
    this.showGate = undefined;
    this.destroyed = false;
  }
  async setUiContent() {}
  async resize(width, height) {
    this.events.push(`resize:${width}x${height}`);
    const gate = this.resizeGate;
    this.resizeGate = undefined;
    if (gate !== undefined) await gate.promise;
  }
  async moveTo(x, y) { this.events.push(`move:${x},${y}`); }
  async show() {
    this.events.push("show");
    const gate = this.showGate;
    this.showGate = undefined;
    if (gate !== undefined) await gate.promise;
  }
  async hide() { this.events.push("hide"); }
  async getCallingWindowInfo() { this.events.push("query"); return { status: 3 }; }
}

const created = [];
const errors = [];
let nextResizeGate;
const preferences = { toolbar: true };
const session = {
  attached: [],
  detached: 0,
  toolbarEnabled: () => preferences.toolbar,
  modeBadgeEnabled: () => false,
  toolbarSize: () => [100, 20],
  toolbarScale: () => 1,
  refreshToolbar() {},
  attach(client) { this.attached.push(client); },
  detach() { this.detached++; },
};
const engine = {
  async createPanel() {
    const panel = new FakeToolbar();
    panel.resizeGate = nextResizeGate;
    nextResizeGate = undefined;
    created.push(panel);
    return panel;
  },
  async destroyPanel(panel) { panel.destroyed = true; panel.events.push("destroy"); },
};
const production = { exports: {} };
vm.runInNewContext(fs.readFileSync(path.join(staging, "compiled/production-methods.js"), "utf8"), {
  exports: production.exports,
  module: production,
  engine,
  inputMethodEngine: { PanelType: { STATUS_BAR: 2 } },
  KeyboardSession: { shared: session },
  ScreenCaptureProtection: { protectPanel() {} },
  KeyboardLog: { error(message) { errors.push(message); }, info() {} },
  describeError: (error) => String(error),
  KeyboardFormFactor: { isDesktop: () => true },
  HarmonyTelemetry: { keyboardShown() {} },
  CandidateCanvasDecoder: { trim() {} },
});
const { Host } = production.exports;

function host() {
  const value = new Host();
  value.moves = [];
  Object.assign(value, {
    tornDown: false,
    toolbar: undefined,
    hud: undefined,
    statusPanelGeneration: 0,
    toolbarPosition: [0, 0],
    toolbarFocus: new ToolbarFocus((error) => errors.push(String(error))),
    toolbarMoves: {
      attach: (panel, position) => value.moves.push(["attach", panel, [...position]]),
      detach: () => value.moves.push(["detach"]),
    },
    density: 2,
    screenWidth: 1000,
    screenHeight: 800,
    panel: { show() {}, hide() {} },
    pending: undefined,
    caret: [0, 0, 0],
    backspaceHold: { reset() {} },
    heldKeys: { reset() {} },
    screenCaptureProtected: () => false,
    applyScreenCaptureProtection() {},
    beginToolbarDrag() {},
    dragToolbar() {},
  });
  return value;
}
const lowerRight = "move:760,600";

(async () => {
  // 正常打开：先调整尺寸、移到右下角、接上移动队列，再显示。
  const plain = host();
  await plain.reconcileStatusPanels();
  const first = created[0];
  assert.deepEqual(first.events, ["resize:200x40", lowerRight, "show"], "工具栏先定位到右下角再显示");
  assert.equal(plain.moves.length, 1);
  assert.equal(plain.moves[0][1], first, "移动队列接上新建的工具栏");
  assert.deepEqual(plain.moves[0][2], [760, 600]);

  // 每次绑定编辑器都请求显示；调用窗口全屏也一样，不查询窗口状态、不隐藏。
  plain.started(undefined, "editor-1");
  await until(() => first.events.length === 4, "绑定编辑器后请求显示");
  plain.started(undefined, "editor-2");
  await until(() => first.events.length === 5, "再次绑定编辑器后再次请求显示");
  assert.deepEqual(first.events.slice(3), ["show", "show"]);
  assert.ok(!first.events.includes("query") && !first.events.includes("hide"), "全屏的调用窗口不查询状态、不隐藏工具栏");

  // 偏好重载时面板正在建：更新的一轮 reconcile 接手这块面板，准备期间到达的焦点被门控丢弃；面板仍要定位到右下角并显示，移动队列也要接上。
  const adopting = host();
  const resize = deferred();
  nextResizeGate = resize;
  const opening = adopting.reconcileStatusPanels();
  await until(() => created.length === 2 && created[1].events.length === 1, "新工具栏开始调整尺寸");
  const adopted = created[1];
  adopting.started(undefined, "editor-during-setup");
  await tick();
  await tick();
  assert.ok(!adopted.events.includes("show"), "准备期间的焦点不能抢先显示面板");
  const reloaded = adopting.reconcileStatusPanels();
  resize.resolve();
  await Promise.all([opening, reloaded]);
  assert.equal(adopting.toolbar, adopted, "更新的一轮接手而不是重建面板");
  assert.ok(!adopted.destroyed, "被接手的面板不能销毁");
  const moved = adopted.events.indexOf(lowerRight);
  const shown = adopted.events.indexOf("show");
  assert.ok(moved >= 0, `被接手的面板仍要移到右下角：${adopted.events.join(",")}`);
  assert.ok(shown > moved, `被接手的面板定位之后显示，首帧不出现在系统默认位置：${adopted.events.join(",")}`);
  assert.ok(adopting.moves.some((move) => move[0] === "attach" && move[1] === adopted), "被接手的面板接上移动队列");
  assert.equal(created.length, 2, "接手时不另建面板");

  // 编辑器解绑后，排在慢显示后面的请求不再执行。
  const unbinding = host();
  await unbinding.reconcileStatusPanels();
  const third = created[2];
  const slow = deferred();
  third.showGate = slow;
  unbinding.started(undefined, "editor-a");
  await until(() => third.events.filter((event) => event === "show").length === 2, "慢显示开始");
  unbinding.started(undefined, "editor-b");
  unbinding.stopped();
  slow.resolve();
  await tick();
  await tick();
  assert.equal(third.events.filter((event) => event === "show").length, 2, "解绑后排队的显示请求不能执行");

  // 关闭工具栏时排队的显示请求作废，移动队列解绑。
  const closing = host();
  await closing.reconcileStatusPanels();
  const fourth = created[3];
  const held = deferred();
  fourth.showGate = held;
  closing.started(undefined, "editor-c");
  await until(() => fourth.events.filter((event) => event === "show").length === 2, "慢显示开始");
  closing.started(undefined, "editor-d");
  preferences.toolbar = false;
  await closing.reconcileStatusPanels();
  preferences.toolbar = true;
  held.resolve();
  await tick();
  await tick();
  assert.ok(fourth.destroyed, "关闭偏好后销毁工具栏");
  assert.equal(fourth.events.filter((event) => event === "show").length, 2, "关闭后排队的显示请求不能执行");
  assert.deepEqual(closing.moves.at(-1), ["detach"], "关闭工具栏时解绑移动队列");

  // 服务创建期间获得焦点的编辑器：面板建好后接上它，并请求显示工具栏。
  const cold = host();
  const coldPanel = cold.panel;
  cold.panel = undefined;
  cold.started(undefined, "editor-cold");
  assert.equal(cold.pending, "editor-cold");
  await cold.reconcileStatusPanels();
  const fifth = created[4];
  const before = fifth.events.filter((event) => event === "show").length;
  cold.panel = coldPanel;
  cold.attachPending(true);
  assert.equal(cold.pending, undefined);
  assert.equal(session.attached.at(-1), "editor-cold");
  await until(() => fifth.events.filter((event) => event === "show").length === before + 1, "待绑定的编辑器接上后请求显示工具栏");

  assert.deepEqual(errors, [], "正常路径不报告错误");
  console.log("Harmony toolbar setup adapter passed");
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
