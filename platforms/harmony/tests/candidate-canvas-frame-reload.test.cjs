// 执行候选画布 `CandidateCanvasFrame` 现有的 dropLease、reload、repaint 方法：换场景时释放旧租约，迟到的旧解码不能覆盖新图，清除场景时释放并清空画布。
// 这里只覆盖画布已有的重载逻辑，不覆盖 `KeyboardView.settings` 的 `@State` 依赖追踪；那一项需要在设备上验收。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { stripTypeScriptTypes } = require('node:module');
const root = path.resolve(__dirname, '../entry/src/main/ets/keyboard') + path.sep;
const frame = fs.readFileSync(root + 'candidate/CandidateCanvasFrame.ets', 'utf8');

function method(source, signature) {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `production method missing: ${signature}`);
  const open = source.indexOf('{', start);
  let depth = 0;
  for (let i = open; i < source.length; i++) {
    if (source[i] === '{') depth++;
    if (source[i] === '}' && --depth === 0) return source.slice(start, i + 1);
  }
  throw new Error(`unterminated method ${signature}`);
}

const resolves = {};
const released = [];
const decoder = {
  retain(scene) {
    return {promise: new Promise(resolve => { resolves[scene.key] = resolve; })};
  },
  release(key, lease) { released.push([key, lease]); }
};
const source = `class Frame {
  ${method(frame, 'private dropLease()')}
  ${method(frame, 'private reload()')}
  ${method(frame, 'private repaint()')}
}`;
const js = stripTypeScriptTypes(source, { mode: 'strip' });
const sandbox = {CandidateCanvasDecoder: decoder, CandidateCanvasPolicy: {}, clearTimeout};
vm.runInNewContext(js + '\nthis.Frame = Frame;', sandbox);
const f = new sandbox.Frame();
f.scene = {key: 'old'}; f.lease = null; f.leaseKey = ''; f.decoded = null;
f.generation = 0; f.timer = -1; f.notified = false; f.ready = true;
f.widthVp = 100; f.heightVp = 60; f.contentInsets = {};
f.stopTimer = () => {}; f.geometry = () => null;
let clearedUnder = 0, clearedOver = 0, playback = 0;
f.under = {clearRect() { clearedUnder++; }}; f.over = {clearRect() { clearedOver++; }};
f.refreshPlayback = () => { playback++; };
const ready = []; f.onPanelReady = (key, value) => ready.push([key, value]);
f.reload(); const oldLease = f.lease;
f.scene = {key: 'new'}; f.reload(); const newLease = f.lease;
assert.notEqual(oldLease, newLease);
assert.deepEqual(released[0], ['old', oldLease]);
assert.deepEqual(ready[0], ['old', false]);
resolves.old({name: 'old bitmap'});
Promise.resolve().then(() => {
  assert.equal(f.decoded, null, 'late old decode must not restore old image');
  resolves.new({name: 'new bitmap'});
  return Promise.resolve();
}).then(() => {
  assert.equal(f.decoded.name, 'new bitmap');
  assert.equal(playback, 1);
  f.scene = null; f.reload();
  assert.equal(f.decoded, null);
  assert.equal(f.lease, null);
  assert.equal(f.leaseKey, '');
  assert.deepEqual(released[1], ['new', newLease]);
  assert.deepEqual(ready[1], ['new', false]);
  assert.ok(clearedUnder >= 3 && clearedOver >= 3, 'scene changes clear both canvases');
  console.log('candidate canvas frame reload passed (ArkUI runtime not exercised)');
}).catch(error => { console.error(error); process.exitCode = 1; });
