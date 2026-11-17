// 执行视图的实际判断与点击方法，防止纯策略通过而宿主仍提前拦截双拼。
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const compiled = path.join(root, "target/harmony-tests/entry/src/main/ets/keyboard");
const { KeyboardScheme } = require(path.join(compiled, "KeyboardScheme.js"));
const { KeyIdPolicy } = require(path.join(compiled, "KeyIdPolicy.js"));
const source = fs.readFileSync(path.join(root,
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"), "utf8");
function method(name) {
  const found = source.match(new RegExp(`^  private ${name}\\([^]*?^  }`, "m"));
  assert.ok(found, `找不到生产方法 ${name}`);
  return found[0];
}
const staging = path.join(root, "target/harmony-tests/shuangpin-separator-adapter");
fs.mkdirSync(staging, { recursive: true });
const input = path.join(staging, "production-methods.ts");
fs.writeFileSync(input, `export class View { ${method("usesQuanpinSeparator")}\n${method("tapLetterShift")} }`);
execFileSync(path.join(root, "apps/desktop/node_modules/.bin/tsc"),
  ["--ignoreConfig", "--noCheck", "--target", "es2021", "--module", "commonjs",
    "--outDir", path.join(staging, "compiled"), input], { stdio: "inherit" });
let composing = false;
const events = [];
const moduleUnderTest = { exports: {} };
vm.runInNewContext(fs.readFileSync(path.join(staging, "compiled/production-methods.js"), "utf8"), {
  exports: moduleUnderTest.exports, module: moduleUnderTest, KeyboardScheme, KeyIdPolicy,
  KeyboardSession: { shared: {
    softKeyPress: (key) => events.push(["key", key]),
    composing: () => composing,
    press: (code, shift) => events.push(["press", code, shift]),
  } },
});
const view = new moduleUnderTest.exports.View();
let touch = true;
view.touchDesign = () => touch;
view.letterCase = { toggle: () => events.push(["toggle"]), mode: () => "upper" };
function check(scheme, shiftKept, expected, boundary = {}) {
  view.scheme = scheme;
  view.quanpinShift = shiftKept;
  view.english = boundary.english ?? false;
  view.symbols = boundary.symbols ?? false;
  view.localMode = boundary.local ?? "none";
  touch = boundary.touch ?? true;
  assert.equal(view.usesQuanpinSeparator(), expected, `${scheme.id} shift=${shiftKept}`);
  for (composing of [false, true]) {
    events.length = 0;
    view.tapLetterShift();
    assert.deepEqual(events, expected
      ? [["key", "Quote"], ...(composing ? [["press", 39, false]] : [])]
      : [["key", KeyIdPolicy.SHIFT], ["toggle"]], `${scheme.id} composing=${composing}`);
    if (!expected) assert.equal(view.caseMode, "upper");
  }
}
for (const scheme of [KeyboardScheme.XIAOHE, KeyboardScheme.ZIRANMA,
  KeyboardScheme.MICROSOFT, KeyboardScheme.SHOUDAO]) {
  for (const shiftKept of [false, true]) {
    check(scheme, shiftKept, true);
    for (const boundary of [{ touch: false }, { english: true }, { symbols: true }, { local: "emoji" }]) {
      check(scheme, shiftKept, false, boundary);
    }
  }
}
check(KeyboardScheme.QUANPIN, false, true);
check(KeyboardScheme.QUANPIN, true, false);
check(KeyboardScheme.QUANPIN_NINE_KEY, false, false);
check(KeyboardScheme.WUBI, false, false);
console.log("鸿蒙双拼视图：全拼开关隔离、触屏边界与实际分词/Shift 派发通过");
