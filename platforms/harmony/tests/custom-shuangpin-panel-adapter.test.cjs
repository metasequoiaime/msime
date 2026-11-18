// 执行真实 ArkUI 面板逻辑和文件适配器；只剥离声明与布局，界面语法由 HAP 编译覆盖。
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const source = path.join(root, "platforms/harmony/entry/src/main/ets");
const staging = path.join(root, "target/harmony-tests/custom-shuangpin-adapter");
fs.mkdirSync(staging, { recursive: true });
function evaluate(text, modules) {
  const input = path.join(staging, "production.ts");
  fs.writeFileSync(input, text);
  execFileSync(path.join(root, "apps/desktop/node_modules/.bin/tsc"), [
    "--ignoreConfig",
    "--noCheck",
    "--target",
    "es2021",
    "--module",
    "commonjs",
    "--outDir",
    path.join(staging, "compiled"),
    input,
  ]);
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(staging, "compiled/production.js"), "utf8"), {
    module,
    exports: module.exports,
    ArrayBuffer,
    Uint8Array,
    require(id) {
      assert.ok(id in modules, id);
      return Object.assign({ __esModule: true }, modules[id]);
    },
  });
  return module.exports;
}
async function main() {
  let bytes = Buffer.from('{"synthetic":true}');
  let offset = 0;
  let closed = 0;
  let opened = 0;
  let advertised = bytes.length;
  let cancelled = false;
  let shortWrite = false;
  let writeFailure = false;
  const written = [];
  const fileSystem = {
    OpenMode: { READ_ONLY: 1, READ_WRITE: 2, CREATE: 4, TRUNC: 8 },
    openSync() {
      opened++;
      offset = 0;
      return { fd: 3 };
    },
    statSync() {
      return { size: advertised };
    },
    readSync(_fd, buffer) {
      const count = Math.min(3, bytes.length - offset, buffer.byteLength);
      new Uint8Array(buffer).set(bytes.subarray(offset, offset + count));
      offset += count;
      return count;
    },
    writeSync(_fd, buffer) {
      if (writeFailure) return 0;
      const count = shortWrite ? Math.min(2, buffer.byteLength) : buffer.byteLength;
      written.push(Buffer.from(buffer).subarray(0, count));
      return count;
    },
    closeSync() {
      closed++;
    },
  };
  const { ShuangpinFiles } = evaluate(
    fs.readFileSync(path.join(source, "keyboard/settings/ShuangpinFiles.ets"), "utf8"),
    {
      "@ohos.app.ability.common": {},
      "@ohos.file.fs": { default: fileSystem },
      "@ohos.file.picker": {
        default: {
          DocumentSelectOptions: class {},
          DocumentSaveOptions: class {},
          DocumentViewPicker: class {
            async select() {
              return cancelled ? [] : ["synthetic://selected"];
            }
            async save() {
              return cancelled ? [] : ["synthetic://exported"];
            }
          },
        },
      },
      "@ohos.util": {
        default: {
          TextDecoder: {
            create() {
              return {
                decodeWithStream(value) {
                  return new TextDecoder("utf-8", { fatal: true }).decode(value);
                },
              };
            },
          },
          TextEncoder: class {
            encodeInto(value) {
              return new TextEncoder().encode(value);
            }
          },
        },
      },
    },
  );
  assert.equal(await ShuangpinFiles.read({}), bytes.toString());
  assert.equal(closed, 1);
  advertised = 16385;
  await assert.rejects(ShuangpinFiles.read({}), /16 KB/);
  assert.equal(closed, 2);
  advertised = 2;
  bytes = Buffer.alloc(16385, 32);
  await assert.rejects(ShuangpinFiles.read({}), /16 KB/);
  assert.equal(closed, 3);
  advertised = 1;
  bytes = Buffer.from([255]);
  await assert.rejects(ShuangpinFiles.read({}));
  assert.equal(closed, 4);
  shortWrite = true;
  assert.equal(await ShuangpinFiles.write({}, "合成 JSON"), true);
  assert.equal(Buffer.concat(written).toString(), "合成 JSON");
  assert.equal(closed, 5);
  writeFailure = true;
  await assert.rejects(ShuangpinFiles.write({}, "synthetic"), /完整/);
  assert.equal(closed, 6);
  cancelled = true;
  const openedBefore = opened;
  assert.equal(await ShuangpinFiles.read({}), undefined);
  assert.equal(await ShuangpinFiles.write({}, "synthetic"), false);
  assert.equal(opened, openedBefore);

  const actions = [];
  const dialogs = [];
  let left = 0;
  class Model {
    constructor() {
      this.ready = true;
      this.dirty = false;
      this.keys = [];
      this.problem = "";
      this.message = "";
    }
    load() {
      actions.push("load");
    }
    fields() {
      return [];
    }
    template(profile) {
      actions.push(profile);
    }
    remove() {
      actions.push("delete");
    }
    importText(value) {
      actions.push(value);
    }
    exportText() {
      return "synthetic-export";
    }
  }
  const original = fs.readFileSync(path.join(source, "pages/CustomShuangpinPanel.ets"), "utf8");
  const panelSource = (original.slice(0, original.indexOf("  @Builder")) + "}")
    .replace("@Component", "")
    .replace("export struct ", "export class ")
    .replace(/@Prop @Watch\('[^']+'\)/g, "")
    .replace(/@State /g, "");
  let picked = undefined;
  const { CustomShuangpinPanel } = evaluate(panelSource, {
    "@ohos.app.ability.common": {},
    "../keyboard/settings/CustomShuangpinSettings": { CustomShuangpinSettings: Model },
    "../keyboard/settings/ShuangpinFiles": {
      ShuangpinFiles: {
        async read() {
          return picked;
        },
        async write(_context, text) {
          actions.push(text);
          return true;
        },
      },
    },
    "../keyboard/AppEdition": { AppEdition: { current: () => ({ id: "full" }) } },
  });
  const panel = new CustomShuangpinPanel();
  panel.getUIContext = () => ({
    getHostContext: () => ({}),
    showAlertDialog: (options) => dialogs.push(options),
  });
  panel.back = () => left++;
  panel.aboutToAppear();
  assert.deepEqual(actions, ["load"]);
  panel.askBack();
  assert.equal(left, 1);
  panel.model.dirty = true;
  panel.askBack();
  assert.equal(left, 1);
  dialogs.pop().primaryButton.action();
  assert.equal(left, 1);
  panel.askBack();
  dialogs.pop().secondaryButton.action();
  assert.equal(left, 2);
  panel.remove();
  assert.ok(!actions.includes("delete"));
  dialogs.pop().primaryButton.action();
  assert.ok(!actions.includes("delete"));
  panel.remove();
  dialogs.pop().secondaryButton.action();
  assert.ok(actions.includes("delete"));
  panel.chooseTemplate(1);
  assert.ok(!actions.includes("ziranma"));
  dialogs.pop().secondaryButton.action();
  assert.ok(actions.includes("ziranma"));
  const beforeImport = actions.slice();
  panel.requestImport();
  const importConfirmation = dialogs.pop();
  assert.match(importConfirmation.title, /替换当前草稿/);
  importConfirmation.primaryButton.action();
  assert.deepEqual(actions, beforeImport);
  await panel.importFile();
  assert.ok(!actions.includes(undefined));
  picked = "synthetic-import";
  await panel.importFile();
  assert.ok(actions.includes("synthetic-import"));
  await panel.exportFile();
  assert.ok(actions.includes("synthetic-export"));
  panel.busy = true;
  panel.askBack();
  assert.equal(left, 2);
  console.log("自定义双拼原生确认、导入导出、容量边界、UTF-8、短写及文件关闭通过");
}
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
