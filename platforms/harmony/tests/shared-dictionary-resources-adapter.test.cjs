// 执行实际 ArkTS 资源适配器，只替换文件系统与 NAPI。
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { execFileSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const directory = path.join(root, "platforms/harmony/entry/src/main/ets/keyboard");
const staging = path.join(root, "target/harmony-tests/shared-resource-adapter");
fs.mkdirSync(staging, { recursive: true });
for (const name of ["SharedDictionaryResources.ets", "SharedDictionaryResourcePolicy.ts"]) {
  fs.copyFileSync(path.join(directory, name), path.join(staging, name.replace(".ets", ".ts")));
}
const configuredTsc = path.join(root, "apps/desktop/node_modules/.bin/tsc");
const tsc = fs.existsSync(configuredTsc) ? configuredTsc : "tsc";
execFileSync(
  tsc,
  [
    "--ignoreConfig",
    "--noCheck",
    "--target",
    "es2021",
    "--module",
    "commonjs",
    "--outDir",
    path.join(staging, "compiled"),
    path.join(staging, "SharedDictionaryResources.ts"),
    path.join(staging, "SharedDictionaryResourcePolicy.ts"),
  ],
  { stdio: "inherit" },
);
function compile(name, dependencies) {
  const module = { exports: {} };
  vm.runInNewContext(
    fs.readFileSync(path.join(staging, "compiled", `${name}.js`), "utf8"),
    {
      module,
      exports: module.exports,
      require(id) {
        assert.ok(id in dependencies, `未提供替身：${id}`);
        const dependency = dependencies[id];
        return Object.hasOwn(dependency, "default")
          ? { __esModule: true, ...dependency }
          : dependency;
      },
    },
    { filename: name },
  );
  return module.exports;
}
const policy = compile("SharedDictionaryResourcePolicy", {});
const shared = "/synthetic/group/msime/dictionary-resources";
const local = "/synthetic/settings/state";
const legacy = "/synthetic/group/msime/settings/state";
const packs = new Map();
const calls = [];
const failures = new Set();
const ids = ["japanese", "wubi", "settled-model"];
function set(root, pack) {
  packs.set(`${root}/resource-packs/${pack}`, true);
}
function response(ok = true) {
  return JSON.stringify({ ok, value: { removed: true } });
}
let release;
const barrier = new Promise((resolve) => {
  release = resolve;
});
const native = {
  resourcePacks(document) {
    const query = JSON.parse(document);
    return JSON.stringify({
      ok: true,
      value: ids.map((id) => ({
        id,
        state: packs.has(`${query.state_root}/resource-packs/${id}`) ? "installed" : "missing",
        size: 42,
        schemes: ["synthetic"],
        import_files: [{ name: "synthetic.dat" }],
      })),
    });
  },
  async resourcePackAdopt(document) {
    const query = JSON.parse(document);
    calls.push(["adopt", query]);
    await barrier;
    set(query.state_root, query.pack);
    return response();
  },
  resourcePackRemove(document) {
    const query = JSON.parse(document);
    calls.push(["remove", query]);
    if (failures.has(query.state_root)) return response(false);
    packs.delete(`${query.state_root}/resource-packs/${query.pack}`);
    return response();
  },
};
const { SharedDictionaryResources } = compile("SharedDictionaryResources", {
  "@ohos.file.fs": {
    default: {
      lstatSync(file) {
        if (!packs.has(file)) throw new Error("synthetic missing");
        return {
          isDirectory() {
            return true;
          },
        };
      },
    },
  },
  "libmsimeclient.so": { default: native },
  "./SharedStateMirror": {},
  "./KeyboardLog": {
    KeyboardLog: {
      error(message) {
        throw new Error(message);
      },
    },
    describeError: String,
  },
  "./SharedDictionaryResourcePolicy": policy,
});
const mirror = { dictionaryResourceRoot: () => shared, legacyDictionaryResourceRoot: () => legacy };
async function run() {
  set(local, "japanese");
  set(legacy, "wubi");
  set(local, "settled-model");
  const adapter = new SharedDictionaryResources(mirror, local);
  const first = adapter.prepare();
  assert.equal(adapter.prepare(), first, "并发请求等待同一次迁移");
  release();
  await first;
  assert.ok(packs.has(`${shared}/resource-packs/japanese`));
  assert.ok(packs.has(`${shared}/resource-packs/wubi`));
  assert.ok(!packs.has(`${local}/resource-packs/japanese`));
  assert.ok(!packs.has(`${legacy}/resource-packs/wubi`));
  const list = JSON.parse(adapter.list());
  assert.equal(list.value.find((item) => item.id === "japanese").state, "installed");
  assert.equal(list.value.find((item) => item.id === "settled-model").state, "installed");
  assert.equal(list.value[0].size, 42);
  assert.deepEqual(list.value[0].import_files, [{ name: "synthetic.dat" }], "合并保留资源元信息");
  assert.equal(adapter.rootForPack("japanese"), shared);
  assert.equal(adapter.rootForPack("wubi"), shared);
  assert.equal(adapter.rootForPack("settled-model"), local);
  const count = calls.filter(([kind]) => kind === "adopt").length;
  await adapter.prepare();
  assert.equal(calls.filter(([kind]) => kind === "adopt").length, count);
  set(local, "japanese");
  set(legacy, "japanese");
  failures.add(local);
  assert.equal(JSON.parse(adapter.remove("japanese")).ok, false, "私有旧副本清理失败要报告失败");
  assert.ok(packs.has(`${shared}/resource-packs/japanese`));
  assert.ok(packs.has(`${legacy}/resource-packs/japanese`));
  failures.clear();
  failures.add(legacy);
  assert.equal(JSON.parse(adapter.remove("japanese")).ok, false, "旧副本清理失败不能报告卸载成功");
  assert.ok(packs.has(`${shared}/resource-packs/japanese`), "清理旧副本失败保留当前可用资源");
  failures.clear();
  assert.equal(JSON.parse(adapter.remove("japanese")).ok, true);
  await adapter.prepare();
  assert.equal(
    JSON.parse(adapter.list()).value.find((item) => item.id === "japanese").state,
    "missing",
  );
  assert.ok(packs.has(`${local}/resource-packs/settled-model`));
  assert.equal(JSON.parse(adapter.remove("settled-model")).ok, true);
  assert.equal(calls.at(-1)[1].state_root, local, "其他资源包仍从私有路径卸载");
  set(local, "wubi");
  failures.add(shared);
  const before = calls.length;
  assert.equal(JSON.parse(adapter.remove("wubi")).ok, false);
  assert.ok(calls.length > before);
  assert.ok(packs.has(`${shared}/resource-packs/wubi`), "共享卸载失败保留当前资源且报告失败");
  failures.clear();
  const keyboard = "/synthetic/keyboard/state";
  set(keyboard, "wubi");
  const cleanupStart = calls.length;
  SharedDictionaryResources.removePrivateCopies(mirror, keyboard);
  assert.ok(!packs.has(`${keyboard}/resource-packs/wubi`));
  assert.ok(
    calls
      .slice(cleanupStart)
      .every(([kind, query]) => kind !== "remove" || query.state_root === keyboard),
  );
  console.log("鸿蒙实际资源适配器：迁移并发、列表元信息、目录分流、卸载和键盘清理通过");
}
run().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
