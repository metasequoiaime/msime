// 运行真实行 Builder，按 Row 的份额与键距测量键帽，防止短排重新拉伸单位宽度。
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const source = path.join(root, "platforms/harmony/entry/src/main/ets/keyboard");
const compiled = path.join(root, "target/harmony-tests/entry/src/main/ets/keyboard");
const staging = path.join(root, "target/harmony-tests/letter-width-row-adapter");
fs.mkdirSync(staging, { recursive: true });
const { FourteenKeyLayout } = require(path.join(compiled, "input/FourteenKeyLayout.js"));
const { EighteenKeyLayout } = require(path.join(compiled, "input/EighteenKeyLayout.js"));
const { fourteenKeyLetterWidthPlan } = require(
  path.join(compiled, "input/FourteenKeyLetterWidths.js"),
);
function method(text, name) {
  const found = text.match(new RegExp(`^  (?:private )?${name}\\([^]*?^  }`, "m"));
  assert.ok(found, name);
  return found[0];
}
function builders(text) {
  return text
    .replace(/^\s*\/\/.*$/gm, "")
    .replace(
      /^(\s*)Row\(([^\n]*)\) \{/gm,
      (_match, indent, args) => `${indent}Row(${args || "{}"}, () => {`,
    )
    .replace(/^([ \t]*)}\s*\n([ \t]*)\./gm, "$1})\n$2.");
}

function evaluate(text, globals, dependencies = {}) {
  const input = path.join(staging, "builder.ts");
  fs.writeFileSync(input, builders(text));
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
  vm.runInNewContext(fs.readFileSync(path.join(staging, "compiled/builder.js"), "utf8"), {
    module,
    exports: module.exports,
    ...globals,
    require(id) {
      assert.ok(id in dependencies, id);
      return dependencies[id];
    },
  });
  return module.exports;
}
let stack = [];
let roots = [];
function Row(options = {}, build) {
  if (typeof options === "function") {
    build = options;
    options = {};
  }
  const node = {
    kind: "row",
    children: [],
    space: options.space ?? 0,
    weight: 0,
    inset: 0,
    width: "100%",
  };
  (stack.length ? stack[stack.length - 1].children : roots).push(node);
  stack.push(node);
  build();
  stack.pop();
  const api = {
    width(value) {
      node.width = value;
      return api;
    },
    height() {
      return api;
    },
    offset(value) {
      node.offset = value.x;
      return api;
    },
    layoutWeight(value) {
      node.weight = value;
      return api;
    },
    padding(value) {
      node.inset = value.left + value.right;
      return api;
    },
  };
  return api;
}
const globals = {
  Row,
  ForEach(cells, render) {
    cells.forEach(render);
  },
  KeyboardMetrics: { SMALL_KEY_FONT_SIZE: 14, MODIFIER_WIDTH_VP: 40 },
  NineKeyLayout: { separator: () => ({ description: "分词" }) },
  KeyAccessibilityPolicy: { symbolPanel: () => "符" },
  FUNCTION_LABEL_WEIGHT: 500,
};
const rowPath = path.join(source, "faces/LetterWidthRow.ets");
if (fs.existsSync(rowPath)) {
  const body = fs
    .readFileSync(rowPath, "utf8")
    .replace("@Component", "")
    .replace("export struct ", "export class ")
    .replace(/@Require\s*/g, "")
    .replace(/@Prop\s*/g, "")
    .replace(/@BuilderParam\s*/g, "");
  const { LetterWidthRow } = evaluate(body, globals, {
    "../input/FourteenKeyLetterWidths": {},
  });
  globals.LetterWidthRow = (options) => {
    const row = new LetterWidthRow();
    Object.assign(row, options);
    row.build();
  };
}
const keyboard = fs.readFileSync(path.join(source, "KeyboardView.ets"), "utf8");
const methods = ["fourteenKeyLetterRow", "eighteenKeyLetterRow"];
for (const name of ["fourteenKeyLetterCell", "eighteenKeyLetterCell"]) {
  if (keyboard.includes(`  private ${name}(`)) methods.push(name);
}
const { View } = evaluate(
  `export class View { ${methods.map((name) => method(keyboard, name)).join("\n")} }`,
  globals,
);
function leaf(key, weight, kind = "key") {
  stack[stack.length - 1].children.push({ kind, key, weight });
}
function measure(node, width, measured = []) {
  if (node.kind !== "row") {
    measured.push({ ...node, width });
    return measured;
  }
  if (typeof node.width === "string" && node.width.startsWith("calc(")) {
    width += Number(node.width.match(/\+\s*([\d.]+)vp/)[1]);
  }
  const inner = width - node.inset - (node.children.length - 1) * node.space;
  const total = node.children.reduce((sum, child) => sum + child.weight, 0);
  for (const child of node.children) measure(child, (inner * child.weight) / total, measured);
  return measured;
}
for (const [layout, rows] of [
  ["fourteen", FourteenKeyLayout.rows()],
  ["eighteen", EighteenKeyLayout.rows()],
]) {
  for (const mode of ["equal", "varied"])
    for (const width of [180, 320, 600])
      for (const gap of [0, 3, 6]) {
        const view = new View();
        view.fourteenKeyLetterWidths = mode;
        view.keySpacing = () => gap;
        view.rowHeight = () => 48;
        view.faceKey = () => "synthetic";
        view.fourteenKeySeparates = () => false;
        view.editing = "";
        view.fourteenKeyLetterPlan = view.eighteenKeyLetterPlan = () =>
          fourteenKeyLetterWidthPlan(rows, mode, 1.4);
        view.fourteenKeyButton = view.eighteenKeyButton = (key, share) => leaf(key, share);
        view.fourteenKeySpacer = view.eighteenKeySpacer = (share, owner) =>
          leaf(owner, share, "pad");
        view.fixedKey = (options) => leaf(null, options.share, "edge");
        view.deleteKey = (_width, _gaps, share) => leaf(null, share, "edge");
        const firstWeight = rows[0].reduce(
          (sum, key) => sum + (key.letters.length > 1 ? 1 : 0.75),
          0,
        );
        const unit = (width + gap) / firstWeight;
        for (const rowIndex of [0, 1, 2]) {
          stack = [];
          roots = [];
          view[`${layout}KeyLetterRow`](rowIndex);
          assert.equal(roots.length, 1);
          const measured = measure(roots[0], width);
          assert.equal(measured.filter((key) => key.kind === "key").length, rows[rowIndex].length);
          for (const key of measured.filter((item) => item.kind === "key")) {
            const share = key.key.letters.length > 1 ? 1 : 0.75;
            const plan = fourteenKeyLetterWidthPlan(rows, mode, 1.4)[rowIndex];
            const equalWidth =
              (width - (plan.length - 1) * gap) / plan.reduce((sum, cell) => sum + cell.weight, 0);
            const difference =
              mode === "varied" ? key.width + gap - share * unit : key.width - equalWidth;
            assert.ok(
              Math.abs(difference) < 1e-8,
              `${layout} 第 ${rowIndex + 1} 排 ${key.key.letters}，宽 ${width}、距 ${gap}：键距份额不同`,
            );
          }
        }
      }
}
console.log("鸿蒙 14/18 键：真实行 Builder 在窄屏、全宽及三档键距下保持共同单位宽度");
