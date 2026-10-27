const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

// 去掉真实排程方法的访问修饰符和返回类型，隔离 ArkUI、NAPI 和真实计时器；队列结果由测试逐轮推进。
const source = fs.readFileSync(path.join(__dirname,
  '../entry/src/main/ets/keyboard/KeyboardSession.ets'), 'utf8');
const methods = ['schedulePersonalDictionaryDrain', 'scheduleDictionarySnapshotPoll',
  'cancelDictionarySnapshotPoll', 'pauseIdleDictionaryWork'];
const bodies = methods.map(name => {
  const match = source.match(new RegExp(`^  private ${name}\\(\\): void \\{[\\s\\S]*?^  \\}`, 'm'));
  assert.ok(match, `找不到会话方法 ${name}`);
  return match[0];
}).join('\n');
const javascript = `class Session { ${bodies.replace(/\bprivate /g, '').replace(/\): void/g, ')')} }; Session;`;

function harness() {
  const timers = new Map();
  let nextTimer = 0;
  let rebuilds = 0;
  const Session = vm.runInNewContext(javascript, {
    setTimeout: callback => { timers.set(++nextTimer, callback); return nextTimer; },
    clearTimeout: timer => timers.delete(timer),
    IdleDictionaryRetryPolicy: { delay: () => 2000 },
  });
  const session = new Session();
  Object.assign(session, {
    handle: 1, inputClient: {}, idleDictionaryVisible: true,
    personalDictionarySettled: false, personalDictionaryTimer: -1,
    personalDictionaryStalls: 0, snapshotQueueTimer: -1, snapshotQueueStalls: 0,
    localMode: 'none', composing: () => false,
    restartIdleSession() {
      rebuilds++;
      this.personalDictionaryStalls++;
      this.schedulePersonalDictionaryDrain();
      this.scheduleDictionarySnapshotPoll();
    },
  });
  return { session, timers, rebuilds: () => rebuilds,
    tick() {
      const [id, callback] = timers.entries().next().value;
      timers.delete(id);
      callback();
    } };
}

const hiddenProgress = harness();
hiddenProgress.session.schedulePersonalDictionaryDrain();
hiddenProgress.session.pauseIdleDictionaryWork();
assert.equal(hiddenProgress.timers.size, 1, '隐藏时仍允许有进展的导入完成下一批');
hiddenProgress.tick();
assert.equal(hiddenProgress.rebuilds(), 1);
assert.equal(hiddenProgress.timers.size, 0, '隐藏后才停滞的队列不能再次排程，也不能重新启动快照查询');
hiddenProgress.session.idleDictionaryVisible = true;
hiddenProgress.session.schedulePersonalDictionaryDrain();
assert.equal(hiddenProgress.timers.size, 1, '重新显示时恢复卡住队列的退避重试');

const hiddenStall = harness();
hiddenStall.session.personalDictionaryStalls = 2;
hiddenStall.session.schedulePersonalDictionaryDrain();
hiddenStall.session.scheduleDictionarySnapshotPoll();
hiddenStall.session.pauseIdleDictionaryWork();
assert.equal(hiddenStall.timers.size, 0, '隐藏时取消已经停滞的重试和快照查询');

const shownStall = harness();
shownStall.session.personalDictionaryStalls = 2;
shownStall.session.schedulePersonalDictionaryDrain();
assert.equal(shownStall.timers.size, 1, '显示时卡住的队列仍可重试');
console.log('idle dictionary lifecycle: hidden progress stops on the first stall and resumes when shown');
