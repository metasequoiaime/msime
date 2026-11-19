const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const outputRoot = process.env.MSIME_HARMONY_TOOLBAR_OUT;
if (!outputRoot) {
  throw new Error('MSIME_HARMONY_TOOLBAR_OUT is required');
}
function load(relative) {
  const compiled = path.join(outputRoot, relative.replace(/\.ts$/, '.js'));
  const module = { exports: {} };
  vm.runInNewContext(`(function(exports, module, require) { ${fs.readFileSync(compiled, 'utf8')}\n})(module.exports, module, require);`, { module, require });
  return module.exports;
}
const { ToolbarDragGesture } = load('keyboard/ToolbarDragGesture.ts');
const { ToolbarMoveQueue } = load('inputmethodextability/ToolbarMoveQueue.ts');
const drag = new ToolbarDragGesture();
const ev = (x, y, ox, oy) => ({ fingerList: [{ displayX: x, displayY: y }], offsetX: ox, offsetY: oy });
const equal = (actual, expected) => assert.deepEqual(Array.from(actual), expected);
drag.start(ev(100, 200, 1, 1));
equal(drag.update(ev(120, 210, 3, 2)), [20, 10]); // 不依赖随窗口移动而变化的相对坐标
equal(drag.update(ev(150, 190, 2, 1)), [50, -10]);
assert.equal(drag.update({ fingerList: [], offsetX: 5, offsetY: 7 }), null);
drag.start({ fingerList: [], offsetX: 1, offsetY: 2 });
equal(drag.update({ fingerList: [], offsetX: 8, offsetY: 12 }), [7, 10]);
const calls = [], resolvers = [], errors = [];
const panel = { moveTo(x, y) { calls.push([x, y]); return new Promise((resolve, reject) => resolvers.push({ resolve, reject })); } };
const queue = new ToolbarMoveQueue(e => errors.push(e));
const tick = () => new Promise(resolve => setImmediate(resolve));
(async () => {
  queue.attach(panel, [0, 0]);
  queue.move([1, 2]); queue.move([3, 4]); queue.move([5, 6]);
  assert.deepEqual(calls, [[1, 2]], 'only one move in flight');
  resolvers.shift().resolve(); await tick();
  assert.deepEqual(calls, [[1, 2], [5, 6]], 'intermediate positions coalesce');
  queue.move([7, 8]);
  equal(queue.anchor(), [7, 8]);
  // 旧移动尚未完成时重新按下，要保留最近请求的位置。
  resolvers.shift().resolve(); await tick();
  assert.deepEqual(calls.at(-1), [7, 8]);
  resolvers.shift().resolve(); await tick();
  equal(queue.anchor(), [7, 8]);
  // 旧请求未完成时重新按下，以实际可见位置作为起点。
  queue.move([30, 30]); queue.move([80, 80]);
  queue.rebase([7, 8]);
  resolvers.shift().resolve(); await tick();
  assert.deepEqual(calls.at(-1), [7, 8], 'a stationary re-press cancels stale queued movement');
  resolvers.shift().resolve(); await tick();
  queue.move([30, 30]); queue.move([80, 80]);
  queue.rebase([7, 8]); queue.move([2, 8]);
  resolvers.shift().resolve(); await tick();
  assert.deepEqual(calls.at(-1), [2, 8], 'opposite-direction drag uses visible starting position');
  resolvers.shift().resolve(); await tick();
  queue.move([9, 10]); queue.move([11, 12]);
  resolvers.shift().reject(Error('move failed')); await tick();
  assert.equal(errors.length, 1); assert.deepEqual(calls.at(-1), [11, 12], 'failure does not wedge queue');
  queue.move([13, 14]); queue.detach(); queue.attach(panel, [0, 0]); queue.move([15, 16]);
  assert.deepEqual(calls.at(-1), [15, 16], 'replacement panel does not wait for stale move');
  resolvers.shift().resolve(); await tick();
  assert.deepEqual(calls.at(-1), [15, 16], 'stale pending move cannot affect replacement');
  resolvers.shift().resolve(); await tick();
  console.log('toolbar drag: screen coordinates and bounded move queue passed');
})().catch(error => { console.error(error); process.exitCode = 1; });
