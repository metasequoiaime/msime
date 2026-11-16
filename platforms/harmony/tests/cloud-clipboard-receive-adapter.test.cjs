// 执行真实 ArkTS 接收器，只替换系统剪贴板、网络和时钟。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { execFileSync } = require('node:child_process');
const root = path.resolve(__dirname, '../../..');
const source = path.join(root, 'platforms/harmony/entry/src/main/ets/keyboard/clipboard');
const staging = path.join(root, 'target/harmony-tests/cloud-clipboard-adapter');
fs.mkdirSync(staging, { recursive: true });
for (const name of ['CloudClipboardAutoReceive', 'CloudClipboardSystem']) {
  fs.copyFileSync(path.join(source, `${name}.ets`), path.join(staging, `${name}.ts`));
}
execFileSync(path.join(root, 'apps/desktop/node_modules/.bin/tsc'), [
  '--ignoreConfig', '--noCheck', '--target', 'es2021', '--module', 'commonjs',
  '--outDir', path.join(staging, 'compiled'),
  path.join(staging, 'CloudClipboardAutoReceive.ts'), path.join(staging, 'CloudClipboardSystem.ts'),
], { stdio: 'inherit' });
const { CloudClipboardReceivePolicy } = require(path.join(root,
  'target/harmony-tests/entry/src/main/ets/keyboard/clipboard/CloudClipboardReceivePolicy.js'));
const { CloudClipboardState, CloudClipboardPolicy, CloudClipboardSendOutcome } = require(path.join(root,
  'target/harmony-tests/entry/src/main/ets/keyboard/clipboard/CloudClipboardPolicy.js'));
function load(name, dependencies, timers = {}) {
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(staging, 'compiled', `${name}.js`), 'utf8'), {
    module, exports: module.exports, require(name) {
      assert.ok(name in dependencies, `缺少替身 ${name}`);
      return dependencies[name];
    }, ...timers,
  });
  return module.exports;
}
const item = hour => ({ id: 'a'.repeat(64), text: `合成远端文本 ${hour}`,
  updatedAt: `2026-10-01T${hour}:00:00Z`, pinned: false, device: 'Synthetic device' });
function deferred() {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
}
const settle = () => new Promise(done => setImmediate(done));
function fixture() {
  const state = { local: '合成本机文本', binding: 'synthetic-account', visible: true, allowed: true,
    rows: [item(10)], writes: [], uploads: [], sendFails: false, timers: new Map(), listGate: null, readGate: null, readCount: 0 };
  let nextTimer = 1;
  const { CloudClipboardAutoReceive } = load('CloudClipboardAutoReceive', {
    './CloudClipboardReceivePolicy': { CloudClipboardReceivePolicy },
    './CloudClipboardPolicy': { CloudClipboardState, CloudClipboardPolicy },
  }, { setTimeout(callback) { const id = nextTimer++; state.timers.set(id, callback); return id; },
    clearTimeout(id) { state.timers.delete(id); } });
  const receiver = new CloudClipboardAutoReceive({
    visible: () => state.visible, readAllowed: async () => state.allowed,
    list: async () => state.listGate ? state.listGate.promise : {
      binding: state.binding, state: CloudClipboardState.READY, items: state.rows },
    matches: binding => binding === state.binding,
    read: async () => { state.readCount++; return state.readGate ? state.readGate.promise : state.local; },
    write: async text => { state.writes.push(text); state.local = text; return true; },
    send: async (text, binding) => {
      state.uploads.push({ text, binding });
      if (state.sendFails) throw new Error('synthetic network failure');
      return CloudClipboardSendOutcome.SENT;
    },
  });
  async function tick() {
    const entry = state.timers.entries().next().value;
    assert.ok(entry, '接收器应排下一次轮询');
    state.timers.delete(entry[0]); entry[1](); await settle();
  }
  receiver.start();
  return { state, receiver, tick };
}
(async () => {
  {
    const { state, tick, receiver } = fixture();
    await tick(); assert.equal(state.writes.length, 0);
    assert.equal(state.uploads.length, 0, '初始未知年代的本机文本不上传');
    state.rows = [item(11)]; await tick();
    assert.deepEqual(state.writes, [item(11).text]);
    await tick(); assert.equal(state.writes.length, 1);
    assert.equal(state.uploads.length, 0, '接收回声不得回传');
    state.local = '合成新本机复制'; state.rows = [item(12)]; await tick();
    assert.equal(state.writes.length, 1);
    assert.equal(state.uploads[0].text, '合成新本机复制');
    state.rows = [item(13)]; await tick(); assert.equal(state.writes.length, 2);
    receiver.stop();
  }
  {
    const { state, tick, receiver } = fixture();
    await tick();
    state.local = '合成发送失败'; state.sendFails = true;
    await tick(); await tick();
    assert.equal(state.uploads.length, 1, '失败后不延后重传旧复制');
    state.sendFails = false; state.local = '合成新的复制';
    await tick(); assert.equal(state.uploads.length, 2, '新复制可独立重试');
    state.binding = 'synthetic-other-account'; state.local = '合成换号前复制';
    await tick(); assert.equal(state.uploads.length, 2, '已有内容不自动发到刚换的账号');
    state.local = 'x'.repeat(4001); await tick(); assert.equal(state.uploads.length, 2);
    receiver.stop();
  }
  for (const change of ['stop', 'restart', 'hide', 'deny', 'account', 'local']) {
    const { state, tick, receiver } = fixture();
    await tick();
    const gate = deferred(); state.listGate = gate;
    await tick();
    if (change === 'stop') receiver.stop();
    if (change === 'restart') { receiver.stop(); receiver.start(); }
    if (change === 'hide') state.visible = false;
    if (change === 'deny') state.allowed = false;
    if (change === 'account') state.binding = 'synthetic-other-account';
    if (change === 'local') state.local = '合成并发复制';
    gate.resolve({ binding: 'synthetic-account', state: CloudClipboardState.READY, items: [item(11)] });
    await settle();
    assert.equal(state.writes.length, 0, `迟到请求必须服从 ${change}`);
    if (change === 'restart') assert.equal(state.timers.size, 1, '旧请求不能改写新生命周期的调度');
    receiver.stop();
  }
  {
    const { state, tick, receiver } = fixture();
    state.local = null;
    await tick(); state.rows = [item(11)]; await tick();
    assert.equal(state.writes.length, 0, '非文本或读取失败不能当作空白覆盖');
    receiver.stop();
  }
  for (const kind of ['empty', 'image', 'error', 'text']) {
    const pasteboard = { MIMETYPE_TEXT_PLAIN: 'text/plain', getSystemPasteboard: () => ({
      getData: async () => {
        if (kind === 'error') throw new Error('synthetic refusal');
        return { getRecordCount: () => kind === 'empty' ? 0 : 1,
          hasType: () => kind === 'text', getPrimaryText: () => '合成文本' };
      },
    }) };
    const { cloudClipboardSystemText } = load('CloudClipboardSystem', {
      '@ohos.pasteboard': { __esModule: true, default: pasteboard },
    });
    assert.equal(await cloudClipboardSystemText(), kind === 'empty' ? '' : kind === 'text' ? '合成文本' : null);
  }
  console.log('Harmony 云剪贴板真实接收器：水位、并发复制、生命周期、绑定及系统读取边界通过');
})().catch(error => { console.error(error); process.exitCode = 1; });
