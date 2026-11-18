// 执行真实原生面板、页面生命周期和桥接方法；布局语法由 HAP 编译检查。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { execFileSync } = require('node:child_process');
const root = path.resolve(__dirname, '../../..');
const sources = path.join(root, 'platforms/harmony/entry/src/main/ets');
const compiled = path.join(root, 'target/harmony-tests/entry/src/main/ets');
const staging = path.join(root, 'target/harmony-tests/touch-shuangpin-adapter');
fs.mkdirSync(staging, { recursive: true });
function evaluate(source, dependencies, globals = {}) {
  const input = path.join(staging, 'methods.ts');
  fs.writeFileSync(input, source);
  execFileSync(path.join(root, 'apps/desktop/node_modules/.bin/tsc'), ['--ignoreConfig', '--noCheck', '--target', 'es2021',
    '--module', 'commonjs', '--outDir', path.join(staging, 'compiled'), input], { stdio: 'inherit' });
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(staging, 'compiled/methods.js'), 'utf8'), {
    module, exports: module.exports, require(id) { assert.ok(id in dependencies); return dependencies[id]; }, ...globals
  });
  return module.exports;
}
const original = fs.readFileSync(path.join(sources, 'pages/TouchShuangpinPanel.ets'), 'utf8');
const panelSource = (original.slice(0, original.indexOf('  build()')) + '}')
  .replace('@Component', '').replace('export struct ', 'export class ')
  .replace(/@Prop @Watch\('refresh'\)/g, '').replace(/@State /g, '');
const { TouchShuangpinPanel } = evaluate(panelSource, {
  '../keyboard/AppEdition': { AppEdition: { current: () => ({ id: 'full' }) } },
  '../keyboard/settings/TouchShuangpinSettings': require(path.join(compiled, 'keyboard/settings/TouchShuangpinSettings.js'))
});
let profile = 'xiaohe'; let revision = 4; let writes = 0;
const panel = new TouchShuangpinPanel();
panel.bridge = {
  loadPreferences() { return JSON.stringify({ok:true,value:{format_version:1,revision,preferences:{profile}}}); },
  plan(document) {
    const request = JSON.parse(document);
    return JSON.stringify({ok:true,value:{options:[{profile:'sogou',title:'搜狗双拼',touch_scheme:'sogou'}],
      profile:request.profile || request.preferences.profile, preferences:{profile:request.profile || request.preferences.profile}}});
  },
  savePreferences(expected, document) {
    assert.equal(expected, revision); writes++; revision++;
    profile = JSON.parse(document).preferences.profile;
    return panel.bridge.loadPreferences();
  }
};
panel.aboutToAppear();
assert.equal(writes, 0);
assert.equal(panel.options.length, 1);
panel.select('sogou');
assert.equal(panel.profile, 'sogou');
assert.equal(writes, 1);
profile = 'xiaohe'; panel.refresh();
assert.equal(panel.profile, 'xiaohe', '前台刷新回读外部修改');
const hostSource = fs.readFileSync(path.join(sources, 'pages/Settings.ets'), 'utf8');
function method(name) {
  const result = hostSource.match(new RegExp(`^  (?:private )?${name}\\([^]*?^  }`, 'm'));
  assert.ok(result, name); return result[0];
}
const entry = {};
const { Host } = evaluate(`export class Host { ${method('aboutToAppear')} ${method('closeShuangpin')} ${method('onBackPress')} }`, {}, {
  HarmonyPrivacyConsent: { networkAllowed: () => true }, EntryAbility: entry, setTimeout: () => {}, LAUNCH_COVER_LIMIT_MS: 1
});
const host = new Host();
host.bridge = new Proxy({}, { get(target, name) { return target[name] || (() => {}); } });
host.getUIContext = () => ({}); host.loadNotices = () => {}; host.openPrivacyDialog = () => {};
host.aboutToAppear();
assert.equal(host.privacyGranted, true, '已有授权在重开页面时恢复，不依赖再弹一次授权框');
let forced = 0; let webBack = 0;
host.bridge.refreshPreferencesIfChanged = force => { assert.equal(force, true); forced++; };
host.controller = { accessBackward: () => true, backward: () => webBack++ };
host.showShuangpin = true;
assert.equal(host.onBackPress(), true);
assert.equal(host.showShuangpin, false);
assert.equal(forced, 1); assert.equal(webBack, 0);
assert.equal(host.onBackPress(), true); assert.equal(webBack, 1);
const revisionPolicy = require(path.join(compiled, 'keyboard/input/PreferenceRevisionPolicy.js'));
const delivered = [];
const { Bridge } = evaluate(`export class Bridge { ${method('refreshPreferencesIfChanged')} }`, {}, {
  client: { loadPreferences: () => JSON.stringify({ok:true,value:{revision}}) },
  PreferenceRevisionPolicy: revisionPolicy.PreferenceRevisionPolicy
});
const bridge = new Bridge(); bridge.observedRevision = revision;
bridge.webview = { runJavaScript(script) { delivered.push(script); return Promise.resolve(); } };
bridge.refreshPreferencesIfChanged(); assert.equal(delivered.length, 0);
bridge.refreshPreferencesIfChanged(true); assert.equal(delivered.length, 1);
console.log('双拼原生面板、前台刷新、返回、授权恢复和网页快照刷新通过');
