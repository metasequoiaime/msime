// 执行实际会话方法，用合成文件与引擎回答模拟同名图片更新。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {stripTypeScriptTypes} = require('node:module');
const root = path.resolve(__dirname, '../entry/src/main/ets/keyboard');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const policy = read('candidate/CandidateCanvasPolicy.ts').replace(/export /g, '');
const identity = read('candidate/CandidateSkinPackageIdentity.ts').replace(/export /g, '');
const toolbarPolicy = read('ToolbarImagePolicy.ts').replace(/^import .*$/gm, '').replace(/export /g, '');
const source = read('KeyboardSession.ets');
function method(name) {
  const found = source.match(new RegExp(`^  (?:private )?${name}\\([^]*?^  }`, 'm'));
  assert.ok(found, `找不到会话方法 ${name}`);
  return found[0];
}
const applyStart = source.indexOf('  private applyAppearance(preferences: PreparedPreferences): void {');
const applyEnd = source.indexOf('    // The store validates both', applyStart);
assert.ok(applyStart >= 0 && applyEnd > applyStart, '找不到外观资源加载段');
// 工具栏图片在同一个方法的末尾读取，也一并执行，确认它不会在读取后覆盖身份记录。
const toolbarStart = source.indexOf('    this.toolbarImage = this.readToolbarImage(', applyEnd);
const toolbarEnd = source.indexOf('    this.toolbarSkin = ', toolbarStart);
assert.ok(toolbarStart > applyEnd && toolbarEnd > toolbarStart, '找不到工具栏图片加载段');
const apply = source.slice(applyStart, applyEnd) + source.slice(toolbarStart, toolbarEnd) +
  '    this.testSnapshot = { skinImage, backgroundImage, candidateCanvas, toolbarImage: this.toolbarImage };\n  }';
const program = `${policy}\n${identity}\n${toolbarPolicy}\nclass KeyboardSession {\n${method('readCandidateCanvas')}\n${method('readToolbarImage')}\n${method('reloadReplacedSkinPackages')}\n${apply}\n}`;
const files = new Map();
const prefix = '/synthetic/skins/sample/';
function change(name, bytes, revision) {
  files.set(prefix + name, {ino:'1', mtime:String(revision), size:bytes.length, bytes});
}
change('light.png', 'light one', 1);
change('dark.png', 'dark one', 1);
change('ornament.png', 'ornament one', 1);
const pkg = {id:'sample',canvas:true,
  borderImage:{source:'dark.png',lightSource:'light.png',sliceDip:[4],fill:true},
  decorations:[{image:'ornament.png',anchor:'right',offsetXDip:0,offsetYDip:0,widthDip:10,heightDip:10,zIndex:1},
    {image:'light.png',anchor:'left',offsetXDip:0,offsetYDip:0,widthDip:10,heightDip:10,zIndex:2}]};
let afterRead = null;
let desktop = true;
const client = {skinResource(document) {
  const request = JSON.parse(document);
  const item = files.get(prefix + request.relative);
  if (!item) return JSON.stringify({ok:false});
  const bytes = Buffer.from(item.bytes).toString('base64');
  if (afterRead && afterRead.name === request.relative) { const callback = afterRead.run; afterRead = null; callback(); }
  return JSON.stringify({ok:true,value:{contentType:'image/png',bytesBase64:bytes}});
}};
const catalog = {
  package: (packages, id) => packages.find(item => item.id === id) ?? null,
  minWidthVp: () => null, decoration: () => null, background: () => null,
  toolbarStylesheet: () => null, animation: () => ({}),
  base64ByteLength: text => Buffer.from(text,'base64').length,
};
const context = {client, CandidateSkinCatalogPolicy:catalog,
  KeyboardFormFactor:{isDesktop:() => desktop}, GlobalTheme:{candidateColors:() => ({})},
  CustomKeyboardSkin:{from:() => null}, NO_CANDIDATE_SKIN_IMAGE:{image:null},
  KeyboardLog:{info(){}}, JSON, Number, Buffer};
const Session = vm.runInNewContext(`${stripTypeScriptTypes(program)}; KeyboardSession`, context);
Session.statSkinFile = file => files.get(file) ?? null;
const session = new Session();
const prefs = {global_theme:'custom',custom_theme:{candidate_skin:'sample'},candidate_theme:'light',theme:'light'};
Object.assign(session, {handle:7,preparedPreferences:prefs,themeCatalog:[1],stateDirectory:'/synthetic',
  canvasGeneration:0, selectedSkinPackageIdentity:() => 'stable',
  resolveTheme:p => ({candidate_skin:p.custom_theme?.candidate_skin ?? null,candidate:{}}),
  themeDarkFor:mode => mode === 'dark',readCandidateSkinCatalog:() => [pkg],
  readCandidateSkinToolbar:() => null,readCandidateSkinImage:() => ({image:null}),
  refreshToolbar:() => {session.refreshes++;},refreshes:0});
const image = name => session.testSnapshot.candidateCanvas?.resources.find(row => row.relative === name)?.base64;
session.applyAppearance(prefs);
assert.deepEqual(Array.from(session.skinImagePaths), [prefix+'light.png',prefix+'ornament.png']);
assert.equal(Buffer.from(image('light.png'),'base64').toString(),'light one');
assert.equal(session.reloadReplacedSkinPackages(), false);
change('dark.png','dark changed',2);
assert.equal(session.reloadReplacedSkinPackages(), false, '未绘制的深色面板更新不触发浅色重读');
change('light.png','light changed',2);
assert.equal(session.reloadReplacedSkinPackages(), true);
assert.equal(Buffer.from(image('light.png'),'base64').toString(),'light changed');
change('ornament.png','new ornament',2);
assert.equal(session.reloadReplacedSkinPackages(), true);
files.delete(prefix+'light.png');
assert.equal(session.reloadReplacedSkinPackages(), true);
assert.equal(image('light.png'), undefined);
change('light.png','restored',3);
assert.equal(session.reloadReplacedSkinPackages(), true);
assert.equal(Buffer.from(image('light.png'),'base64').toString(),'restored');
// 加载期间变更，身份必须保持加载前的值，下一次检查才能补读。
afterRead = {name:'light.png', run:() => change('light.png','later change',4)};
session.applyAppearance(prefs);
assert.equal(session.reloadReplacedSkinPackages(), true);
assert.equal(Buffer.from(image('light.png'),'base64').toString(),'later change');
// 模式和主题切换不能保留前一个面板的身份和资源。
prefs.candidate_theme='dark'; session.applyAppearance(prefs);
assert.deepEqual(Array.from(session.skinImagePaths), [prefix+'dark.png',prefix+'ornament.png',prefix+'light.png']);
assert.equal(Buffer.from(image('dark.png'),'base64').toString(),'dark changed');
desktop=false; session.applyAppearance(prefs);
assert.equal(session.skinImagePaths.length,0, '触屏模式不引入新的桌面文件检查');
desktop=true; pkg.canvas=false; session.applyAppearance(prefs);
assert.equal(session.testSnapshot.candidateCanvas,null);
assert.equal(session.skinImagePaths.length,0);
pkg.canvas=true;
prefs.custom_theme.candidate_skin=null; session.applyAppearance(prefs);
assert.equal(session.testSnapshot.candidateCanvas,null);
assert.equal(session.skinImagePaths.length,0);
// 带图片工具栏的包：工具栏图与候选贴图共用读取前的同一次身份记录，读取后不再重取。
const text = base64 => Buffer.from(base64,'base64').toString();
const toolbar = name => session.testSnapshot.toolbarImage?.resources.find(row => row.relative === name)?.base64;
prefs.custom_theme.candidate_skin='sample'; prefs.candidate_theme='light';
change('bar.png','bar one',1);
change('key.png','key one',1);
pkg.toolbarImage = {image:'bar.png',imageWidthDip:100,imageHeightDip:20,buttonDefaults:{background:'key.png'},buttons:{}};
session.applyAppearance(prefs);
assert.deepEqual(Array.from(session.skinImagePaths),
  [prefix+'light.png',prefix+'ornament.png',prefix+'bar.png',prefix+'key.png']);
assert.equal(text(toolbar('bar.png')),'bar one');
assert.equal(session.reloadReplacedSkinPackages(), false);
afterRead = {name:'light.png', run:() => change('light.png','toolbar race',5)};
session.applyAppearance(prefs);
assert.equal(session.reloadReplacedSkinPackages(), true, '有工具栏图片时，贴图读取期间的变更也要在下次检查时发现');
assert.equal(text(image('light.png')),'toolbar race');
afterRead = {name:'key.png', run:() => change('key.png','key changed',2)};
session.applyAppearance(prefs);
assert.equal(session.reloadReplacedSkinPackages(), true, '工具栏图片读取期间的变更要在下次检查时发现');
assert.equal(text(toolbar('key.png')),'key changed');
assert.equal(session.reloadReplacedSkinPackages(), false);
desktop=false; session.applyAppearance(prefs);
assert.equal(session.skinImagePaths.length,0, '触屏模式不检查工具栏图片');
assert.equal(session.testSnapshot.toolbarImage,null);
desktop=true;
console.log('候选贴图资源：同名更新、缺失恢复、加载竞态、工具栏图片和主题切换通过');
