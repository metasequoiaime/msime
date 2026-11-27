// 执行设置桥的实际导入方法，文件选择器和系统接口用合成对象替代。
const assert = require('node:assert/strict');
const fsNode = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {stripTypeScriptTypes} = require('node:module');
const root = path.resolve(__dirname, '../entry/src/main/ets');
const source = fsNode.readFileSync(path.join(root, 'pages/Settings.ets'), 'utf8');
function method(name) {
  const match = source.match(new RegExp(`^  (?:private )?(?:async )?${name}\\([^]*?^  }`, 'm'));
  assert.ok(match, `找不到设置桥方法 ${name}`);
  return match[0];
}
const policy = fsNode.readFileSync(path.join(root, 'keyboard/skin/SkinImportPolicy.ts'), 'utf8')
  .replace(/export /g, '');
const code = `${policy}\nclass Bridge {\n${method('importSkinFolder')}\n${method('runSkinImport')}\n}`;
let selected = ['file://docs/skin/sakura'];
let answer = {ok:true, value:{id:'sakura'}};
const directories = new Set(['/state','/state/skins','/cache','/cache/skin-import']);
const copies = [], toasts = [], removed = [], imports = [], closed = [], errors = [];
let allowed = true;
let failOpen = false;
let finishStaging;
let blockStage = true;
let failImport = false;
let failToast = false;
const picker = {DocumentSelectOptions: class {}, DocumentSelectMode: {FOLDER:1},
  DocumentViewPicker: class { async select() { return selected; } }};
const fs = {accessSync: p => directories.has(p), mkdirSync: p => directories.add(p),
  OpenMode: {READ_ONLY:0,DIR:1,NOFOLLOW:2},
  openSync: (p, mode) => {
    assert.equal(mode, 3);
    if (p.startsWith('file://')) {
      assert.equal(p, 'file://docs/skin/sakura');
      if (failOpen) throw new Error('unsupported provider');
      return {fd:100};
    }
    return {fd:101};
  },
  closeSync: file => closed.push(file.fd)};
const client = {async skinImport(arg) { imports.push(JSON.parse(arg));
    if (failImport) throw new Error('import bridge crashed');
    return JSON.stringify(answer); },
  async stageSkinFolder(a,b) { copies.push([a,b]);
    if (blockStage) await new Promise(resolve => {finishStaging = resolve;});
    return allowed; }};
const context = {picker, fs, client, JSON, Number, Error,
  canIUse: () => true, util: {generateRandomUUID: () => 'fixture-id'},
  fileUri: {FileUri: class { constructor() { this.path = '/skin/sakura'; }}},
  KeyboardLog: {info() {}, error: message => errors.push(message)},
  describeError: error => error.message,
  StagedResources: {removeDirectory: p => removed.push(p)}};
const Bridge = vm.runInNewContext(`${stripTypeScriptTypes(code)}; Bridge`, context);
const bridge = new Bridge();
let published = 0;
Object.assign(bridge, {context:{cacheDir:'/cache'}, stateDirectory:'/state', sharedMirror:{},
  safeDirectory: () => true, schedulePublish: () => published++,
  uiContext:{getPromptAction: () => ({showToast: ({message}) => {
    if (failToast) { failToast = false; throw new Error('toast host gone'); }
    toasts.push(message);
  }})}});
const wait = () => new Promise(resolve => setImmediate(resolve));
(async () => {
  assert.equal(JSON.parse(bridge.importSkinFolder()).ok, true);
  await wait();
  assert.deepEqual(copies, [[100,101]]);
  assert.equal(closed.length,0, '异步复制完成前不能关闭选择器 fd');
  finishStaging(); blockStage = false; await wait();
  assert.deepEqual(closed, [101,100]);
  assert.equal(imports.length,1);
  assert.deepEqual(imports[0],{source:'/cache/skin-import/fixture-id/sakura',directory:'/state/skins'});
  assert.equal(copies.length,1); assert.equal(published,1);
  assert.ok(toasts.pop().includes('已导入')); assert.equal(removed.length,1);
  selected = []; bridge.importSkinFolder(); await wait();
  assert.equal(published,1); assert.equal(toasts.length,0); assert.equal(imports.length,1);
  selected = ['file://docs/skin/sakura']; answer = {ok:false,error:'skin_manifest'};
  bridge.importSkinFolder(); await wait();
  assert.equal(published,1); assert.ok(toasts.pop().includes('skin.toml')); assert.equal(removed.length,2);
  allowed = false; errors.length = 0; bridge.importSkinFolder(); await wait();
  assert.equal(imports.length,2); assert.equal(published,1);
  assert.ok(toasts.pop().includes('文件提供方')); assert.equal(removed.length,3);
  assert.deepEqual(errors, ['skin folder not staged: native copy refused the folder'],
    '原生暂存拒绝时必须留下与打开失败可区分的日志');
  failOpen = true; errors.length = 0; bridge.importSkinFolder(); await wait();
  assert.equal(imports.length,2); assert.equal(published,1);
  assert.ok(toasts.pop().includes('文件提供方')); assert.equal(removed.length,4);
  assert.deepEqual(errors, ['skin folder staging failed: unsupported provider'],
    '选择器 URI 打开失败时日志必须带底层错误');
  failOpen = false; allowed = true; answer = {ok:true,value:{id:'sakura'}};
  bridge.sharedMirror = null; bridge.importSkinFolder(); await wait();
  assert.equal(imports.length,3); assert.equal(published,1);
  assert.ok(toasts.pop().includes('无法同步')); assert.equal(removed.length,5);
  failImport = true; errors.length = 0; bridge.importSkinFolder(); await wait();
  assert.equal(imports.length,4); assert.equal(toasts.pop(), '导入失败'); assert.equal(removed.length,6);
  assert.deepEqual(errors, ['skin import failed: import bridge crashed'], '导入异常日志必须带底层错误');
  failImport = false; failToast = true; errors.length = 0; bridge.importSkinFolder(); await wait();
  assert.equal(toasts.pop(), '导入皮肤失败');
  assert.deepEqual(errors, ['skin import failed: toast host gone'], '反馈阶段异常日志必须带底层错误');
  console.log('设置桥：URI 目录 fd、异步生命周期、取消和明确失败反馈通过');
})().catch(error => { console.error(error); process.exitCode = 1; });
