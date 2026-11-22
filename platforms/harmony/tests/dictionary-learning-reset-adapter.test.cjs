// 执行真实会话方法和维护策略；只替换 NAPI 与系统能力，不复制路由逻辑。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { execFileSync } = require('node:child_process');
const root = path.resolve(__dirname, '../../..');
const keyboard = path.join(root, 'platforms/harmony/entry/src/main/ets/keyboard');
const staging = path.join(root, 'target/harmony-tests/dictionary-learning-reset');
fs.mkdirSync(staging, { recursive: true });
const source = fs.readFileSync(path.join(keyboard, 'KeyboardSession.ets'), 'utf8');
const method = source.match(/^  dictionary\(actionDocument: string, resources: string, stateRoot: string\): string \{[\s\S]*?^  \}/m);
assert.ok(method, '找不到真实词库维护方法');
fs.writeFileSync(path.join(staging, 'Session.ts'), `class KeyboardSession {\n${method[0]}\n}; KeyboardSession;`);
fs.copyFileSync(path.join(keyboard, 'DictionaryMaintenancePolicy.ts'), path.join(staging, 'Policy.ts'));
fs.copyFileSync(path.join(keyboard, 'DictionaryLearningResetRequest.ts'), path.join(staging, 'ResetRequest.ts'));
execFileSync(path.join(root, 'apps/desktop/node_modules/.bin/tsc'), [
  '--ignoreConfig', '--noCheck', '--target', 'es2021', '--module', 'commonjs',
  '--outDir', path.join(staging, 'compiled'), path.join(staging, 'Session.ts'), path.join(staging, 'Policy.ts'), path.join(staging, 'ResetRequest.ts'),
], { stdio: 'inherit' });
const policy = { exports: {} };
vm.runInNewContext(fs.readFileSync(path.join(staging, 'compiled/Policy.js'), 'utf8'), { exports: policy.exports });
const resetRequest = { exports: {} };
vm.runInNewContext(fs.readFileSync(path.join(staging, 'compiled/ResetRequest.js'), 'utf8'), { exports: resetRequest.exports });
const javascript = fs.readFileSync(path.join(staging, 'compiled/Session.js'), 'utf8');
function harness({ composing = false, active = true, failure = '', prepareFailure = false } = {}) {
  const calls = [];
  const options = { api_version: 1, marker: 'synthetic-options' };
  const response = JSON.stringify({ ok: true, value: { reset: true, collections_requeued: 0 } });
  const native = {
    prepareHost(document) {
      calls.push(['prepare', JSON.parse(document)]);
      return JSON.stringify(prepareFailure ? { ok: false, error: 'synthetic-prepare' } : { ok: true, value: options });
    },
    dictionaryLearningReset(document) {
      calls.push(['reset', JSON.parse(document)]);
      if (failure === 'throw') throw new Error('synthetic native failure');
      return failure === 'reject' ? JSON.stringify({ ok: false, error: 'dictionary_reset_rejected' }) : response;
    },
    dictionary(document) { calls.push(['legacy', JSON.parse(document)]); return response; },
    personalDictionaryRequest(document) { calls.push(['queue', JSON.parse(document)]); return response; },
    focus(handle, focus) { calls.push(['focus', handle, focus]); },
    setEnglishMode(handle, english) { calls.push(['english', handle, english]); },
    setKeyGrid(handle, grid) { calls.push(['grid', handle, grid]); },
  };
  const Session = vm.runInNewContext(javascript, {
    client: native, DictionaryMaintenancePolicy: policy.exports.DictionaryMaintenancePolicy,
    validLearningResetAction: resetRequest.exports.validLearningResetAction,
    KeyboardLog: { error: message => calls.push(['error', message]) }, describeError: String,
  });
  Session.prepareRequest = (resources, stateRoot) => ({ resources, stateRoot });
  const session = new Session();
  Object.assign(session, {
    handle: active ? 7 : 0, englishMode: true, keyGrid: 9, systemDark: true, inputClient: {},
    composing: () => composing,
    stop() { calls.push(['stop']); this.handle = 0; },
    start(...args) { calls.push(['start', ...args]); this.handle = 8; this.englishMode = false; this.keyGrid = 0; return true; },
    apply() {}, applyView() {}, refreshTypingEffects() {},
    schedulePersonalDictionaryDrain() { calls.push(['drain']); }, personalDictionaryTimer: -1,
  });
  return { session, calls, options, response,
    run(operation = 'reset') { return session.dictionary(JSON.stringify({ operation }), '/synthetic/resources', '/synthetic/state'); } };
}
function count(h, type) { return h.calls.filter(call => call[0] === type).length; }
for (const active of [true, false]) {
  const h = harness({ active });
  assert.equal(h.run(), h.response);
  assert.deepEqual(h.calls.find(call => call[0] === 'reset')[1], { directory: '/synthetic/state', options: h.options });
  assert.equal(count(h, 'legacy'), 0);
  assert.equal(count(h, 'stop'), Number(active));
  assert.equal(count(h, 'start'), Number(active));
  assert.equal(h.session.englishMode, true);
  assert.equal(h.session.keyGrid, 9);
  if (active) {
    assert.ok(h.calls.findIndex(call => call[0] === 'stop') < h.calls.findIndex(call => call[0] === 'reset'));
    assert.equal(count(h, 'focus'), 1);
    assert.equal(count(h, 'english'), 1);
    assert.equal(count(h, 'grid'), 1);
  }
}
for (const extra of ['options', 'directory', 'payload', 'login']) {
  const h = harness();
  const result = h.session.dictionary(JSON.stringify({ operation: 'reset', [extra]: 'synthetic' }),
    '/synthetic/resources', '/synthetic/state');
  assert.equal(JSON.parse(result).ok, false);
  assert.equal(h.calls.length, 0, '额外动作字段必须在释放会话或清除前拒绝');
  assert.equal(h.session.handle, 7);
}
const busy = harness({ composing: true });
assert.equal(JSON.parse(busy.run()).error, 'dictionary maintenance busy');
assert.equal(busy.calls.length, 0, '组字中不能停止会话或调用清除');
for (const failure of ['reject', 'throw']) {
  const h = harness({ failure });
  assert.equal(JSON.parse(h.run()).ok, false);
  assert.equal(count(h, 'reset'), 1);
  assert.equal(count(h, 'legacy'), 0, '账号或屏障失败不能回退无屏障清除');
  assert.equal(count(h, 'start'), 1);
  assert.equal(h.session.englishMode, true);
  assert.equal(h.session.keyGrid, 9);
}
const preparation = harness({ prepareFailure: true });
assert.equal(JSON.parse(preparation.run()).error, 'synthetic-prepare');
assert.equal(count(preparation, 'reset'), 0);
assert.equal(count(preparation, 'start'), 1, '准备失败也恢复空闲会话');
for (const operation of ['list', 'edit', 'import_personal']) {
  const h = harness();
  assert.equal(h.run(operation), h.response);
  assert.equal(count(h, 'reset'), 0);
  assert.equal(count(h, operation === 'import_personal' ? 'queue' : 'legacy'), 1);
  assert.equal(count(h, 'stop'), Number(operation === 'edit'));
}
console.log('鸿蒙真实词库方法：受限清除、失败不回退、组字保护及空闲会话恢复通过');
