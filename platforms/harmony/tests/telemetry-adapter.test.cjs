// 在 Node 中执行实际 ArkTS 适配器；仅替换系统 API 与 NAPI，不复制生命周期实现。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '../../..');
const {execFileSync} = require('node:child_process');
const directory = path.join(root, 'platforms/harmony/entry/src/main/ets/telemetry');
const epoch = '12345678-1234-4234-8234-123456789abc';
const ownSession = 'abcdabcd-abcd-4bcd-abcd-abcdabcdabcd';
const independentCapture = 'feedfeed-feed-4eed-8eed-feedfeedfeed';
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
function compile(file, dependencies, timers = {}) {
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
    setTimeout() { return 1; }, clearTimeout() {}, setInterval() { return 1; }, clearInterval() {}, ...timers,
  }, { filename: file });
  return module.exports;
}
const policy = compile(path.join(directory, 'TelemetryPolicy.ts'), {});
function fixture(options = {}) {
  let nextTimer = 1;
  const state = { reply: JSON.stringify({ok: true, value: {enabled: true, epoch, since_unix_ms: 1000}}),
    network: true, clear: [], end: [], begin: [], flush: [], publications: [], timers: new Map(), intervals: new Map(), writes: [],
    blockedFlush: false, publicationError: false, captureFailure: false, captureBlocked: false, captureSets: [], captureResolves: [], records: [], watcher: null };
  const native = {
    telemetryConsent(text) {
      const query = JSON.parse(text);
      if (query.publication) {
        if (state.publicationError) throw new Error('synthetic publication failure');
        state.publications.push(query);
        // 此替身只描述共享接口的响应，不模拟 Rust 的锁、换代或偏好规则。
        if (!query.publication.policy_allowed) { state.reply = '{"ok":true,"value":{"enabled":false}}'; return state.reply; }
      }
      return state.reply;
    },
    telemetryClear(text) { state.clear.push(JSON.parse(text)); return '{"ok":true,"value":true}'; },
    telemetryEnd(text) { state.end.push(JSON.parse(text)); return '{"ok":true,"value":true}'; },
    telemetryRecordCrash(text) { state.records.push(JSON.parse(text)); return '{"ok":true,"value":true}'; },
    telemetryBegin(text) {
      const request = JSON.parse(text);
      state.begin.push(request);
      const header = 'MSIME-CRASH/1 ' + JSON.stringify({platform:'harmony',version:'1.0.0',revision:'abcdef12',
        run:'7',attempt:'1',arch:'aarch64',component:request.component,consent_epoch:request.consent_epoch}) + '\n';
      return JSON.stringify({ok:true,value:{enabled:true,session_id:request.capture_only ? independentCapture : ownSession,crash_record_header:header}});
    },
    telemetryFlush(text) {
      state.flush.push(JSON.parse(text));
      return state.blockedFlush ? new Promise(() => {}) : Promise.resolve('{"ok":true}');
    },
  };
  const stat = { isSymbolicLink: () => false, isDirectory: () => true, isFile: () => false };
  const io = {accessSync: () => true, lstatSync: () => stat,
    openSync: () => ({fd: 1}), writeSync: (fd, text) => state.writes.push(text), closeSync() {},
    renameSync() {}, OpenMode: {CREATE: 1, WRITE_ONLY: 2, TRUNC: 4}};
  const telemetry = compile(path.join(directory, 'Telemetry.ets'), {
    '../keyboard/NativeCleanup': {NativeCleanup: {close() {}}},
    '@ohos.bundle.bundleManager': {default: {getBundleInfoForSelfSync: () => ({versionName: '1.0.0'}),
      BundleFlag: {GET_BUNDLE_INFO_DEFAULT: 0}}},
    '@ohos.hiviewdfx.hiAppEvent': {default: {removeWatcher() { state.watcher = null; }, addWatcher(watcher) { state.watcher = watcher; },
      setEventParam(parameters,domain,name) {
        state.captureSets.push({parameters,domain,name});
        if (state.captureBlocked) return new Promise(resolve => state.captureResolves.push(resolve));
        return state.captureFailure ? Promise.reject(new Error('synthetic capture failure')) : Promise.resolve();
      },
      domain: {OS: 'os'}, event: {APP_CRASH: 'crash', APP_FREEZE: 'freeze'}}},
    '@ohos.process': {default: {pid: 42}},
    '@ohos.file.fs': {default: io},
    '@ohos.util': {default: {generateRandomUUID: () => 'staging'}},
    'libmsimeclient.so': {default: native},
    '../keyboard/KeyboardLog': {KeyboardLog: {error() {}}, describeError: String},
    './TelemetryPolicy': policy,
    '../privacy/HarmonyPrivacyConsent': {HarmonyPrivacyConsent: {
      networkAllowed: () => state.network, reportingIdentity: () => 'synthetic-policy',
      whenNetworkAllowed: () => new Promise(() => {}),
    }},
  }, {
    setTimeout(fn) { const id = nextTimer++; state.timers.set(id,fn); return id; },
    clearTimeout(id) { state.timers.delete(id); },
    setInterval(fn) { const id = nextTimer++; state.intervals.set(id,fn); return id; },
    clearInterval(id) { state.intervals.delete(id); },
  }).HarmonyTelemetry;
  telemetry.configureAuthority(options.group ?? null, options.publisher ?? true);
  function start() {
    if (options.publisher === false) telemetry.keyboardStarted('/private/state');
    else telemetry.applicationStarted('/private/state');
  }
  function tick() { for (const fn of Array.from(state.intervals.values())) fn(); }
  function begin() { for (const [id,fn] of Array.from(state.timers.entries())) { state.timers.delete(id); fn(); } }
  return {state, telemetry, start, tick, begin};
}
{
  const {state, telemetry, start} = fixture();
  start();
  for (const reply of ['broken', '{"ok":false}', '{"ok":true,"value":{"enabled":false}}',
    '{"ok":true,"value":{"enabled":true}}']) {
    state.reply = reply;
    const request = telemetry.request('/private/state');
    assert.equal(request.consent_epoch, '', '失败路径必须显式绑定拒绝身份');
    assert.equal(request.authority_directory, '/private/state/reporting-authority');
    assert.equal('preferences_directory' in request,false);
    assert.equal('enabled' in request, false, '不能用布尔缓存绕过共享授权');
  }
}
{
  const {state, telemetry, start, tick} = fixture({publisher:false, group:'/group'});
  state.network = false;
  start();
  assert.equal(state.begin.length,1,'键盘直接核对共享政策与统计授权，无需等私有镜像');
  assert.equal(state.begin[0].authority_directory,'/group/msime/reporting-authority');
  assert.equal('preferences_directory' in state.begin[0],false);
  assert.equal(state.publications.length,0,'键盘永远不能发布授权');
  state.reply = '{"ok":true,"value":{"enabled":false}}';
  tick();
  assert.equal(state.clear.length,1,'独立观察器无需焦点就处理撤回');
  assert.equal(state.clear[0].authority_directory,'/group/msime/reporting-authority');
  const newEpoch = '87654321-4321-4321-8321-cba987654321';
  state.reply = JSON.stringify({ok:true,value:{enabled:true,epoch:newEpoch,since_unix_ms:2000}});
  tick();
  assert.equal(state.begin[1].consent_epoch,newEpoch);
  const clears = state.clear.length;
  telemetry.reportingTurnedOff('/private/state');
  assert.equal(state.clear.length,clears,'迟到关闭只能重读，不能删除新队列');
  telemetry.keyboardStopped();
}
{
  const {state, telemetry, start, tick, begin} = fixture({publisher:false, group:'/group'});
  start(); begin();
  const session = telemetry.session;
  for (const reply of ['{"ok":false}', '{"ok":true,"value":{"enabled":false,"suspended":true}}']) {
    state.reply = reply;
    tick();
    assert.equal(state.clear.length,0,'暂停和错误都不能当作撤回清理');
    assert.equal(telemetry.session,session,'暂时暂停保留同代次会话');
  }
  telemetry.keyboardStopped();
  assert.equal(state.end[0].session,ownSession);
}
{
  const {state, telemetry, start, begin} = fixture({publisher:false,group:'/group'});
  start(); begin();
  telemetry.keyboardStopped(); telemetry.keyboardStopped();
  assert.equal(state.end.length,1);
  assert.equal(state.end[0].session,ownSession,'只结束本进程实际开始的会话');
  assert.equal(state.intervals.size,0);
  assert.equal(state.timers.size,0);
}
{
  const {state, telemetry, start, begin} = fixture({publisher:false,group:'/group'});
  start();
  const late = Array.from(state.timers.values())[0];
  telemetry.keyboardStopped();
  late(); begin();
  assert.equal(state.end.length,0,'未开始不结束上一进程会话');
  assert.equal(state.begin.length,1,'停止后的迟到定时器不能重启');
}
{
  const {state, telemetry, start, tick} = fixture({group:'/group'});
  state.blockedFlush = true;
  start();
  assert.equal(state.flush.length,1);
  state.network = false;
  state.reply = '{"ok":true,"value":{"enabled":false}}';
  tick();
  assert.equal(state.publications.at(-1).publication.policy_allowed,false,'网络挂起时仍发布政策撤回');
  assert.equal(state.clear[0].authority_directory,'/group/msime/reporting-authority');
  state.network = true;
  state.reply = JSON.stringify({ok:true,value:{enabled:true,epoch:'87654321-4321-4321-8321-cba987654321',since_unix_ms:2000}});
  telemetry.privacyChanged('/private/state');
  assert.equal(state.flush.length,2,'重开不等待旧网络返回');
  assert.equal(state.begin.every(item => item.capture_only === true),true,'App 不创建输入会话');
  telemetry.applicationStopped();
}
{
  const {state, start, tick} = fixture({publisher:false});
  start(); tick();
  assert.equal(state.begin.length,0,'共享沙箱不可用时键盘不能私自开启');
  assert.equal(state.flush.length,0);
  assert.equal(state.publications.length,0);
}
{
  const {state, telemetry, start} = fixture({group:'/group'});
  state.network = false;
  start();
  assert.equal(state.publications[0].publication.policy_allowed,false,'首次政策未同意也立即发布关闭');
  assert.equal(state.begin.length,0);
  assert.equal(state.flush.length,0);
  telemetry.applicationStopped();
}
{
  const {state, telemetry, start, tick, begin} = fixture({publisher:false,group:'/group'});
  start();
  const oldBegin = Array.from(state.timers.values())[0];
  const next = '87654321-4321-4321-8321-cba987654321';
  state.reply = JSON.stringify({ok:true,value:{enabled:true,epoch:next,since_unix_ms:2000}});
  tick(); oldBegin();
  assert.equal(state.begin.length,2,'只看到最终重开也换代，旧启动不能开始会话');
  begin();
  assert.equal(state.begin[2].consent_epoch,next);
  telemetry.keyboardStopped();
}
{
  const {state, telemetry, start, tick} = fixture({group:'/group'});
  start();
  state.publicationError = true;
  tick();
  assert.equal(telemetry.reportingAllowed(),false,'发布失败不能沿用共享文件里的旧许可');
  assert.equal(state.clear.length,0,'发布失败不等于明确关闭，不能删除队列');
  const flushes = state.flush.length;
  telemetry.send(telemetry.request('/private/state'));
  assert.equal(state.flush.length,flushes);
  state.publicationError = false;
  tick();
  assert.equal(telemetry.reportingAllowed(),true,'同源发布恢复后可继续本代次');
  telemetry.applicationStopped();
}
{
  const {state, telemetry, start, tick} = fixture({publisher:false,group:'/group'});
  state.reply = '{"ok":true,"value":{"enabled":false}}';
  start(); tick(); tick();
  assert.equal(state.clear.length,1,'持续关闭不重复清理磁盘');
  state.reply = JSON.stringify({ok:true,value:{enabled:true,epoch,since_unix_ms:1000}});
  tick();
  state.reply = '{"ok":true,"value":{"enabled":false}}';
  tick();
  assert.equal(state.clear.length,2,'重新开启后再关闭必须重新清理');
  telemetry.keyboardStopped();
}
async function settle() { for (let n=0;n<6;n++) await Promise.resolve(); }
async function captureGroups() {
  {
    const {state,telemetry,start,tick,begin} = fixture({publisher:false,group:'/group'});
    start(); await settle();
    state.captureBlocked = true;
    begin();
    const count = state.captureSets.length;
    assert.equal(JSON.parse(state.captureSets.at(-1).parameters.msime_capture).capture,ownSession);
    tick();
    assert.equal(state.captureSets.length,count,'旧参数未完成前不并发写系统身份');
    telemetry.applicationStopped();
    state.captureBlocked = false;
    state.captureResolves.shift()();
    await settle();
    assert.equal(state.captureSets.at(-1).parameters.msime_capture,'','停止后的迟到设置只能被清空');
    assert.equal(state.begin.length,2,'异步系统收尾不能复活遥测');
    tick();
    assert.equal(state.begin.length,2);
  }
  {
    const {state,telemetry,start,tick} = fixture({publisher:false,group:'/group'});
    start(); await settle();
    const original = state.captureSets.at(-1).parameters.msime_capture;
    assert.equal(JSON.parse(original).capture,independentCapture);
    assert.equal(JSON.parse(original).pid,42);
    state.reply = '{"ok":false}'; tick(); await settle();
    assert.equal(state.captureSets.at(-1).parameters.msime_capture,'','临时读取错误也解除系统参数');
    state.reply = JSON.stringify({ok:true,value:{enabled:true,epoch,since_unix_ms:1000}});
    tick(); await settle();
    assert.equal(state.captureSets.at(-1).parameters.msime_capture,original,'同代次恢复原捕获');
    const next = '87654321-4321-4321-8321-cba987654321';
    state.reply = JSON.stringify({ok:true,value:{enabled:true,epoch:next,since_unix_ms:2000}});
    tick(); await settle();
    assert.equal(JSON.parse(JSON.parse(state.captureSets.at(-1).parameters.msime_capture).header.slice('MSIME-CRASH/1 '.length)).consent_epoch,next);
    telemetry.recordCrash({pid:42,time:Date.now(),version:'1.0.0',message:'SIGABRT',stack:'',capture:original});
    assert.equal(state.records.length,0,'重新开启后迟到的旧系统身份不得追认');
    telemetry.keyboardStopped(); await settle();
  }
  {
    const {state,telemetry,start,tick} = fixture({group:'/group'});
    state.captureFailure = true;
    start(); await settle();
    const calls = state.captureSets.length;
    state.captureFailure = false; tick(); await settle();
    assert.equal(state.captureSets.length,calls+1,'系统参数失败由独立观察器重试');
    const original = state.captureSets.at(-1).parameters.msime_capture;
    const header = JSON.parse(original).header;
    const report = {pid:42,time:Date.now(),version:'1.0.0',message:'SIGABRT',stack:'libsynthetic.so+0x1',capture:original};
    state.watcher.onReceive('os',[{appEventInfos:[{name:'crash',params:{time:report.time,pid:42,bundle_version:'1.0.0',
      crash_type:'NativeCrash',msime_capture:original,exception:{signal:{signo:6},frames:[]}}}]}]);
    assert.equal(state.records[0].diagnostic_capture,independentCapture,'系统崩溃使用原 UUID，而不是 PID 猜会话');
    assert.equal(state.records[0].crash_record_header,header,'原构建头原样传到 Rust');
    assert.equal(state.records[0].outside_session,true);
    assert.equal('pid' in state.records[0],false);
    telemetry.recordCrash({...report,outsideSession:true,message:'Hang THREAD_BLOCK_6S'});
    assert.equal(state.records[1].diagnostic_id,`harmony-freeze-42-${report.time}.crash`);
    assert.equal('diagnostic_capture' in state.records[1],false,'卡死使用独立身份，不占用原输入会话的崩溃文件');
    telemetry.recordCrash({...report,capture:''});
    telemetry.recordCrash({...report,pid:43});
    assert.equal(state.records.length,2,'无原身份和跨进程报告均拒绝');
    telemetry.applicationStopped(); await settle();
  }
  {
    const {state,telemetry,start,tick,begin} = fixture({publisher:false,group:'/group'});
    start(); await settle();
    state.captureBlocked = true; begin();
    const old = state.captureSets.at(-1).parameters.msime_capture;
    const count = state.captureSets.length;
    state.reply = '{"ok":true,"value":{"enabled":false}}'; tick();
    const next = '87654321-4321-4321-8321-cba987654321';
    state.reply = JSON.stringify({ok:true,value:{enabled:true,epoch:next,since_unix_ms:2000}});
    tick();
    assert.equal(state.captureSets.length,count,'撤回重开期间旧请求仍串行等待');
    state.captureBlocked = false; state.captureResolves.shift()(); await settle();
    const latest = state.captureSets.at(-1).parameters.msime_capture;
    assert.equal(JSON.parse(JSON.parse(latest).header.slice('MSIME-CRASH/1 '.length)).consent_epoch,next,'旧设置完成后只发布最终新代次');
    telemetry.recordCrash({pid:42,time:Date.now(),message:'SIGABRT',stack:'',capture:old});
    assert.equal(state.records.length,0,'旧系统参数即使晚装入也不能被新代次认领');
    telemetry.keyboardStopped(); await settle();
  }
  console.log('harmony telemetry adapter: 11 lifecycle and 4 async capture groups passed');
}
captureGroups().catch(error => { console.error(error); process.exitCode=1; });
