// 执行原生面板的实际方法；只剥离 ArkUI 声明和布局，由 HAP 编译覆盖布局语法。
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const tsc = path.join(root, "apps/desktop/node_modules/.bin/tsc");
const staging = path.join(root, "target/harmony-tests/nine-key-flick-adapter");
fs.mkdirSync(staging, { recursive: true });
const sources = path.join(root, "platforms/harmony/entry/src/main/ets");
const compiled = path.join(root, "target/harmony-tests/entry/src/main/ets");
const settings = require(path.join(compiled, "keyboard/settings/FlickThresholdSettings.js"));
const flick = require(path.join(compiled, "keyboard/input/NineKeyFlickPolicy.js"));
const revision = require(path.join(compiled, "keyboard/input/PreferenceRevisionPolicy.js"));
const modes = { Begin: 0, Moving: 1, End: 2, Click: 3 };

function evaluate(source, dependencies, globals = {}) {
  const input = path.join(staging, "production-methods.ts");
  fs.writeFileSync(input, source);
  execFileSync(tsc, ["--ignoreConfig", "--noCheck", "--target", "es2021", "--module", "commonjs",
    "--outDir", path.join(staging, "compiled"), input], { stdio: "inherit" });
  const output = fs.readFileSync(path.join(staging, "compiled/production-methods.js"), "utf8");
  const module = { exports: {} };
  vm.runInNewContext(output, { exports: module.exports, module,
    require(id) {
      assert.ok(id in dependencies, `未提供模块：${id}`);
      return dependencies[id];
    }, SliderChangeMode: modes, ...globals });
  return module.exports;
}

const original = fs.readFileSync(path.join(sources, "pages/NineKeyFlickPanel.ets"), "utf8");
const panelSource = (original.slice(0, original.indexOf("  build()")) + "}\n")
  .replace("@Component", "")
  .replace("export struct ", "export class ")
  .replace(/@Prop @Watch\('reload'\)/g, "")
  .replace(/@State /g, "");
const { NineKeyFlickPanel } = evaluate(panelSource, {
  "../keyboard/settings/FlickThresholdSettings": settings,
  "../keyboard/input/NineKeyFlickPolicy": flick,
});
let current = { format_version: 1, revision: 4, preferences: {} };
let writes = 0;
let failure = false;
const panel = new NineKeyFlickPanel();
panel.loadPreferences = () => JSON.stringify({ ok: true, value: current });
panel.savePreferences = (expected, document) => {
  writes++;
  if (failure) return JSON.stringify({ ok: false, error: "synthetic_failure" });
  assert.equal(expected, current.revision);
  current = JSON.parse(document);
  current.revision++;
  return panel.loadPreferences();
};
panel.aboutToAppear();
assert.equal(panel.threshold, 24);
assert.equal(writes, 0, "打开页面不保存默认值");
panel.change(24, modes.Begin);
panel.change(48, modes.Moving);
assert.equal(panel.threshold, 48, "滑动中预览当前距离");
assert.equal(writes, 0, "开始和拖动不连续写偏好");
panel.change(48, modes.End);
assert.equal(current.preferences.touch_nine_key_flick_threshold, 48);
panel.change(14, modes.Click);
assert.equal(current.preferences.touch_nine_key_flick_threshold, 14, "点按滑块同样保存");
assert.equal(writes, 2);
failure = true;
panel.change(48, modes.End);
assert.equal(panel.threshold, 14, "保存失败恢复权威显示值");
assert.ok(panel.error.length > 0);
current.preferences.touch_nine_key_flick_threshold = 32;
panel.reload();
assert.equal(panel.threshold, 32, "前台刷新回读外部修改");
panel.loadPreferences = () => '{"ok":false}';
panel.reload();
assert.equal(panel.ready, false);
panel.change(48, modes.Click);
assert.equal(writes, 3, "读取失败时不执行保存");

const hostSource = fs.readFileSync(path.join(sources, "pages/Settings.ets"), "utf8");
function method(name) {
  const found = hostSource.match(new RegExp(`^  (?:private )?${name}\\([^]*?^  }`, "m"));
  assert.ok(found, `找不到生产方法 ${name}`);
  return found[0];
}
const { Host } = evaluate(`export class Host { ${method("closeNineKeyFlick")}\n${method("onBackPress")} }`, {});
let nativeClosed = 0;
let webBack = 0;
const host = new Host();
host.bridge = { refreshPreferencesIfChanged(force) { assert.equal(force, true); nativeClosed++; } };
host.controller = { accessBackward: () => true, backward: () => webBack++ };
host.showNineKeyFlick = true;
assert.equal(host.onBackPress(), true);
assert.equal(host.showNineKeyFlick, false);
assert.equal(nativeClosed, 1);
assert.equal(webBack, 0, "系统返回先退出原生面板");
assert.equal(host.onBackPress(), true);
assert.equal(webBack, 1, "没有原生面板时仍返回网页历史");

const delivered = [];
const { Bridge } = evaluate(`export class Bridge { ${method("refreshPreferencesIfChanged")} }`, {}, {
  client: { loadPreferences: () => JSON.stringify({ ok: true, value: current }) },
  PreferenceRevisionPolicy: revision.PreferenceRevisionPolicy,
});
const bridge = new Bridge();
bridge.observedRevision = current.revision;
bridge.webview = { runJavaScript(script) { delivered.push(script); return Promise.resolve(); } };
bridge.refreshPreferencesIfChanged();
assert.equal(delivered.length, 0, "普通前台刷新仍按修订号去重");
bridge.refreshPreferencesIfChanged(true);
assert.equal(delivered.length, 1, "原生面板关闭后把当前快照交还已有共享页");
assert.ok(delivered[0].includes("msimeHarmonyPreferencesChanged"));
assert.ok(delivered[0].includes("touch_nine_key_flick_threshold"));
console.log("鸿蒙 flick 原生适配器：拖动、点按、失败、刷新、系统返回及共享页回读通过");
