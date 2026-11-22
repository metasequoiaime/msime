// 执行 ArkTS 原生面板与真实视图方法，覆盖控件、系统返回和字距单位接线。
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const sources = path.join(root, "platforms/harmony/entry/src/main/ets");
const compiled = path.join(root, "target/harmony-tests/entry/src/main/ets");
const staging = path.join(root, "target/harmony-tests/letter-width-panel-adapter");
fs.mkdirSync(staging, { recursive: true });
const settings = require(path.join(compiled, "keyboard/settings/LetterWidthSettings.js"));
const widths = require(path.join(compiled, "keyboard/input/FourteenKeyLetterWidths.js"));
function evaluate(source, dependencies = {}, globals = {}) {
  const input = path.join(staging, "production-methods.ts");
  fs.writeFileSync(input, source);
  execFileSync(
    path.join(root, "apps/desktop/node_modules/.bin/tsc"),
    [
      "--ignoreConfig",
      "--noCheck",
      "--target",
      "es2021",
      "--module",
      "commonjs",
      "--outDir",
      path.join(staging, "compiled"),
      input,
    ],
    { stdio: "inherit" },
  );
  const module = { exports: {} };
  vm.runInNewContext(
    fs.readFileSync(path.join(staging, "compiled/production-methods.js"), "utf8"),
    {
      exports: module.exports,
      module,
      ...globals,
      require(id) {
        assert.ok(id in dependencies, `未提供模块 ${id}`);
        return dependencies[id];
      },
    },
  );
  return module.exports;
}
function method(source, name, optional = false) {
  const match = source.match(new RegExp(`^  (?:private )?${name}\\([^]*?^  }`, "m"));
  if (!optional) assert.ok(match, `找不到生产方法 ${name}`);
  return match ? match[0] : "";
}
const original = fs.readFileSync(path.join(sources, "pages/KeyboardLetterWidthsPanel.ets"), "utf8");
const panelSource = (original.slice(0, original.indexOf("  build()")) + "}\n")
  .replace("@Component", "")
  .replace("export struct ", "export class ")
  .replace(/@Prop @Watch\('reload'\)/g, "")
  .replace(/@State /g, "");
const { KeyboardLetterWidthsPanel } = evaluate(panelSource, {
  "../keyboard/settings/LetterWidthSettings": settings,
  "../keyboard/input/FourteenKeyLetterWidths": widths,
});
let current = { format_version: 1, revision: 9, preferences: { synthetic_other: 7 } };
let writes = 0;
let fail = false;
const panel = new KeyboardLetterWidthsPanel();
panel.loadPreferences = () => JSON.stringify({ ok: true, value: current });
panel.savePreferences = (revision, document) => {
  writes++;
  if (fail) return '{"ok":false}';
  assert.equal(revision, current.revision);
  current = JSON.parse(document);
  current.revision++;
  return panel.loadPreferences();
};
panel.aboutToAppear();
assert.equal(panel.mode, "equal");
assert.equal(writes, 0, "打开面板不落盘默认值");
panel.select(1);
assert.equal(current.preferences.touch_fourteen_key_letter_widths, "varied");
assert.equal(current.preferences.synthetic_other, 7);
panel.select(0);
assert.equal(panel.mode, "equal");
panel.select(-1);
assert.equal(writes, 2);
current.preferences.touch_fourteen_key_letter_widths = "varied";
panel.reload();
assert.equal(panel.mode, "varied");
fail = true;
panel.select(0);
assert.equal(panel.mode, "varied", "保存失败恢复权威值");
assert.ok(panel.error.length > 0);
panel.loadPreferences = () => '{"ok":false}';
panel.reload();
panel.select(0);
assert.equal(panel.ready, false);
assert.equal(writes, 3);

const host = fs.readFileSync(path.join(sources, "pages/Settings.ets"), "utf8");
const { Host } = evaluate(`export class Host { ${method(host, "onBackPress")}
  ${method(host, "closeKeyboardLetterWidths", true)} ${method(host, "closeNineKeyFlick", true)} }`);
const page = new Host();
let refreshed = 0;
let webBack = 0;
page.bridge = {
  refreshPreferencesIfChanged(force) {
    assert.equal(force, true);
    refreshed++;
  },
};
page.controller = { accessBackward: () => true, backward: () => webBack++ };
page.showKeyboardLetterWidths = true;
page.showNineKeyFlick = false;
assert.equal(page.onBackPress(), true);
assert.equal(page.showKeyboardLetterWidths, false, "系统返回先退出原生宽度面板");
assert.equal(refreshed, 1, "退出后把当前偏好交还共享页");
assert.equal(webBack, 0);
assert.equal(page.onBackPress(), true);
assert.equal(webBack, 1);

const keyboard = fs.readFileSync(path.join(sources, "keyboard/KeyboardView.ets"), "utf8");
const hints = require(path.join(compiled, "keyboard/input/GridLetterHintTable.js"));
const { View } = evaluate(
  `export class View { ${["gridKeyFace", "compactKeyFace", "compactKeyDescription"]
    .map((name) => method(keyboard, name))
    .join("\n")} }`,
  {},
  {
    fourteenKeyLetterSpacing: widths.fourteenKeyLetterSpacing,
    TOUCH_LETTER_FONT_SIZE: 28,
    GridLetterHintTable: hints.GridLetterHintTable,
  },
);
const view = new View();
let taps = 0;
view.tapGridKey = () => taps++;
view.feedback = { letterCornerHints: true };
view.english = false;
view.cornerQuotes = false;
view.hollowBrackets = false;
for (const scheme of ["quanpin", "shuangpin"]) {
  view.scheme = { engineScheme: scheme };
  for (const eighteen of [false, true]) {
    for (const key of [
      { letters: "qw", label: "QW", description: "按键 Q W" },
      { letters: "l", label: "L", description: "按键 L" },
    ]) {
      const options = view.gridKeyFace(key, eighteen);
      const ratio = key.letters === "qw" ? 0.15 : scheme === "shuangpin" ? 0.08 : 0;
      assert.equal(
        options.letterSpacing,
        ratio * options.size,
        "ArkUI 的 fp 字距按实际字号换算 em",
      );
      assert.equal(options.swipeLetters, key.letters);
      options.action();
    }
  }
}
assert.equal(taps, 8);
console.log("鸿蒙键宽原生适配器：控件读写、失败恢复、系统返回与实际字距单位通过");
