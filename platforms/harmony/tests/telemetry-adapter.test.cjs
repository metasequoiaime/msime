// 在 Node 中执行实际 ArkTS 适配器；仅替换系统 API 与 NAPI，不复制生命周期实现。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '../../..');
const {execFileSync} = require('node:child_process');
const directory = path.join(root, 'platforms/harmony/entry/src/main/ets/telemetry');
const epoch = '12345678-1234-4234-8234-123456789abc';
const staging = path.join(root, 'target/harmony-tests/telemetry-adapter');
fs.mkdirSync(staging, {recursive: true});
for (const name of ['Telemetry.ets', 'TelemetryPolicy.ts']) {
  fs.copyFileSync(path.join(directory, name), path.join(staging, name.replace('.ets', '.ts')));
}
// 此处只做发射，系统类型由实际 HAP 编译检查；执行时通过下面的替身替换系统入口。
execFileSync(path.join(root, 'apps/desktop/node_modules/.bin/tsc'), [
  '--ignoreConfig', '--noCheck', '--target', 'es2021', '--module', 'commonjs',
  '--outDir', path.join(staging, 'compiled'),
  path.join(staging, 'Telemetry.ts'), path.join(staging, 'TelemetryPolicy.ts'),
], {stdio: 'inherit'});
function compile(file, dependencies) {
  const name = path.basename(file).replace(/\.(ets|ts)$/, '.js');
  const code = fs.readFileSync(path.join(staging, 'compiled', name), 'utf8');
  const module = { exports: {} };
  vm.runInNewContext(code, {
    exports: module.exports, module,
    require(name) {
      if (!(name in dependencies)) throw new Error(`没有替身: ${name}`);
      const dependency = dependencies[name];
      return Object.hasOwn(dependency, 'default') ? {__esModule: true, ...dependency} : dependency;
    },
    setTimeout() { return 1; }, clearTimeout() {},
  }, { filename: file });
  return module.exports;
}
const policy = compile(path.join(directory, 'TelemetryPolicy.ts'), {});
function fixture() {
  const state = { reply: JSON.stringify({ok: true, value: {enabled: true, epoch, since_unix_ms: 1000}}),
    network: true, clear: [], end: [], begin: [], deleted: [], writes: [] };
  const native = {
    telemetryConsent() { return state.reply; },
    telemetryClear(text) { state.clear.push(JSON.parse(text)); return '{"ok":true,"value":false}'; },
    telemetryEnd(text) { state.end.push(JSON.parse(text)); return '{"ok":true,"value":true}'; },
    telemetryBegin(text) {
      state.begin.push(JSON.parse(text));
      return JSON.stringify({ok: true, value: {enabled: true, session_id: 'own-session'}});
    },
    telemetryFlush() { return Promise.resolve('{"ok":true}'); },
  };
  const stat = { isSymbolicLink: () => false, isDirectory: () => true, isFile: () => false };
  const io = {accessSync: () => true, lstatSync: () => stat,
    unlinkSync: (file) => state.deleted.push(file),
    openSync: () => ({fd: 1}), writeSync: (fd, text) => state.writes.push(text), closeSync() {},
    renameSync() {}, OpenMode: {CREATE: 1, WRITE_ONLY: 2, TRUNC: 4}};
  const telemetry = compile(path.join(directory, 'Telemetry.ets'), {
    '../keyboard/NativeCleanup': {NativeCleanup: {close() {}}},
    '@ohos.bundle.bundleManager': {default: {getBundleInfoForSelfSync: () => ({versionName: '1.0.0'}),
      BundleFlag: {GET_BUNDLE_INFO_DEFAULT: 0}}},
    '@ohos.hiviewdfx.hiAppEvent': {default: {removeWatcher() {}}},
    '@ohos.process': {default: {pid: 42}},
    '@ohos.file.fs': {default: io},
    '@ohos.util': {default: {generateRandomUUID: () => 'staging'}},
    'libmsimeclient.so': {default: native},
    '../keyboard/KeyboardLog': {KeyboardLog: {error() {}}, describeError: String},
    './TelemetryPolicy': policy,
    '../privacy/HarmonyPrivacyConsent': {HarmonyPrivacyConsent: {networkAllowed: () => state.network}},
  }).HarmonyTelemetry;
  telemetry.directory = '/private/state/telemetry';
  telemetry.preferences = '/private/state';
  return {state, telemetry};
}
{
  const {state, telemetry} = fixture();
  for (const reply of ['broken', '{"ok":false}', '{"ok":true,"value":{"enabled":false}}',
    '{"ok":true,"value":{"enabled":true}}']) {
    state.reply = reply;
    const request = telemetry.request('/private/state');
    assert.equal(request.consent_epoch, '', '失败路径必须显式绑定拒绝身份');
    assert.equal(request.preferences_directory, '/private/state');
    assert.equal('enabled' in request, false, '不能用布尔缓存绕过共享授权');
  }
}
{
  const {state, telemetry} = fixture();
  telemetry.enabled = true;
  telemetry.reportingTurnedOff('/private/state');
  assert.equal(state.clear.length, 1);
  assert.equal(state.clear[0].preferences_directory, '/private/state', '迟到关闭必须条件清理');
  assert.equal(telemetry.enabled, true, '迟到关闭不能覆盖已经重新开启的内存状态');
  assert.equal(state.deleted.length, 0);
}
{
  const {state, telemetry} = fixture();
  telemetry.enabled = false;
  state.reply = '{"ok":false,"error":"unavailable"}';
  telemetry.refreshDiagnosticConsent();
  assert.equal(state.deleted.length, 0, '临时读取失败不能删除授权摘要');
  state.reply = '{"ok":true,"value":{"enabled":false}}';
  telemetry.refreshDiagnosticConsent();
  assert.equal(state.deleted.length, 1, '明确关闭才清理授权摘要');
}
{
  const {state, telemetry} = fixture();
  telemetry.begin();
  telemetry.keyboardStopped();
  telemetry.keyboardStopped();
  assert.equal(state.end.length, 1);
  assert.equal(state.end[0].session, 'own-session', '结束必须绑定自己开始的会话');
}
{
  const {state, telemetry} = fixture();
  telemetry.keyboardStopped();
  assert.equal(state.end.length, 0, '尚未开始不能结束上一进程的会话');
  state.network = false;
  telemetry.privacyChanged('/private/state');
  assert.equal(state.clear.length, 1);
  assert.equal('preferences_directory' in state.clear[0], false, '隐私撤回与统计开关独立');
  state.network = true;
  telemetry.privacyChanged('/private/state');
  assert.equal(state.clear.length, 1, '迟到的隐私撤回不能清理重新同意后的队列');
}
console.log('harmony telemetry adapter: 5 lifecycle groups passed');
