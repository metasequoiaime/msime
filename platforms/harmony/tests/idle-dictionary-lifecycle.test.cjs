const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

// 去掉真实排程方法的访问修饰符和返回类型，隔离 ArkUI、NAPI 和真实计时器；队列结果由测试逐轮推进。
const source = fs.readFileSync(path.join(__dirname,
  '../entry/src/main/ets/keyboard/KeyboardSession.ets'), 'utf8');
const methods = ['schedulePersonalDictionaryDrain', 'scheduleDictionarySnapshotPoll',
  'cancelDictionarySnapshotPoll', 'pauseIdleDictionaryWork', 'keyboardShown'];
const bodies = methods.map(name => {
  const match = source.match(new RegExp(`^  (?:private )?${name}\\(\\): void \\{[\\s\\S]*?^  \\}`, 'm'));
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
hiddenProgress.session.keyboardShown();
assert.equal(hiddenProgress.session.idleDictionaryVisible, true);
assert.equal(hiddenProgress.timers.size, 2, '重新显示时恢复卡住队列的退避重试');

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

const resumed = harness();
resumed.session.personalDictionaryStalls = 2;
resumed.session.schedulePersonalDictionaryDrain();
resumed.session.scheduleDictionarySnapshotPoll();
resumed.session.pauseIdleDictionaryWork();
resumed.session.keyboardShown();
assert.equal(resumed.timers.size, 2, '同一编辑框重新显示时恢复个人词库和快照任务');
resumed.session.keyboardShown();
assert.equal(resumed.timers.size, 2, '重复显示事件不能重复排程');
resumed.session.pauseIdleDictionaryWork();
resumed.session.inputClient = undefined;
resumed.session.keyboardShown();
assert.equal(resumed.timers.size, 0, '失焦后迟到的显示事件不能恢复任务');
assert.equal(resumed.session.idleDictionaryVisible, false);
resumed.session.inputClient = {};
resumed.session.handle = 0;
resumed.session.keyboardShown();
assert.equal(resumed.timers.size, 0, '会话销毁后迟到的显示事件不能恢复任务');

const ability = fs.readFileSync(path.join(__dirname,
  '../entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets'), 'utf8');
assert.match(ability, /engine\.on\('keyboardShow', this\.keyboardShown\)/);
assert.match(ability, /engine\.off\('keyboardShow', this\.keyboardShown\)/);
const callback = ability.match(/private readonly keyboardShown:[\s\S]*?= \(\): void => \{([\s\S]*?)\n  \};/);
assert.ok(callback, '找不到显示回调');
let shows = 0;
vm.runInNewContext(callback[1], { KeyboardSession: { shared: { keyboardShown() { shows++; } } } });
assert.equal(shows, 1, '真实显示回调必须通知会话');
