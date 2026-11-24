// 执行实际会话方法并替换引擎调用，检查分页、过滤和候选管理传入的身份。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {stripTypeScriptTypes} = require('node:module');
const root = path.resolve(__dirname, '../entry/src/main/ets/keyboard');
const policy = fs.readFileSync(path.join(root, 'candidate/CandidatePresentationPolicy.ts'), 'utf8')
  .replace(/export /g, '');
const source = fs.readFileSync(path.join(root, 'KeyboardSession.ets'), 'utf8');
function method(name) {
  const pattern = new RegExp(`^  (?:private )?${name}\\([^]*?^  }`, 'm');
  const match = source.match(pattern);
  assert.ok(match, `找不到会话方法 ${name}`);
  return match[0];
}
const names = ['choose', 'selectEdge', 'allCandidates', 'pageCandidateIdentity',
  'promoteCandidate', 'fixCandidatePosition', 'clearCandidatePosition', 'removeCandidate',
  'removeManagedCandidate', 'chooseAny', 'refreshCandidateView'];
const code = `${policy}\nclass Session {\n${names.map(method).join('\n')}\n}`;
const calls = [];
const action = name => (...args) => { calls.push([name, ...args]); return 'ok'; };
const client = {select: action('select'), pinCandidate: action('pin'),
  fixCandidatePosition: action('fix'), clearCandidatePosition: action('clear'),
  removeCandidate: action('remove'), selectAnyCandidate: action('any'),
  view: action('view'), selectEdge: (...args) => { calls.push(['edge', ...args]);
    return JSON.stringify({ok:true,value:{commit:'字'}}); },
  allCandidates: () => JSON.stringify({ok:true,value:{generation:44,candidates:allRows}})};
const context = {client, JSON, Number, CandidateTextEdge: {FIRST: 0},
  CandidateManagementAction: {validatePosition: p => p, candidateActionsAvailable: () => true},
  KeyboardScheme: {engineSchemeName: () => ''}, SchemeTraits: {KOREAN: 1},
  EngineViewValuePolicy: {isObject: x => typeof x === 'object' && x !== null,
    isGeneration: Number.isSafeInteger},
  CandidateGlossPolicy: {annotation: () => '', annotationIsTranslation: () => false,
    slotAnnotation: () => '', hunEum: () => ''},
  TranslationPolicy: {sourceGetsNoGloss: () => false}, CorrectionMarkPolicy: {confident: () => false},
  KeyboardLog: {error: e => {throw new Error(e);}}};
const Session = vm.runInNewContext(`${stripTypeScriptTypes(code)}; Session`, context);
const session = new Session();
Object.assign(session, {handle: 7, generation: 40, translationPage: null,
  currentCandidates: [], visibleCandidateIndices: [], lastView: {scheme:0},
  apply: () => true, applyView: action('applyView'), candidateManagementAllowed: () => true,
  engineCandidateIndex: i => session.visibleCandidateIndices[i] ?? -1,
  candidateListOpen: () => false, asTraditional: s => s});
const row = (index, generation = 42, source = 0) =>
  ({id: {session:7,generation,index}, source, text:`候选${index}`, highlighted:false});
const page = [row(10,42,15),row(11),row(12)];
session.currentCandidates = page;
session.lastView.candidates = page;
session.visibleCandidateIndices = [1,2];
session.choose(0); session.promoteCandidate(1); session.fixCandidatePosition(0, 2);
session.clearCandidatePosition(1); session.removeCandidate(0); session.removeManagedCandidate(1);
page[2].highlighted = true; session.selectEdge(0);
assert.deepEqual(calls.splice(0), [['select',7,42,11],['pin',7,42,12],['fix',7,42,11,2],
  ['clear',7,42,12],['remove',7,42,11],['remove',7,42,12],['edge',7,42,12,0]]);
session.choose(7); session.choose(-1); assert.equal(calls.length, 0);
page[1].id.session = 8; session.choose(0); assert.equal(calls.length, 0);
const allRows = [row(0,44,15),row(1,44),row(20,44)];
assert.equal(session.allCandidates().length, 2);
session.chooseAny(1); assert.deepEqual(calls.splice(0), [['any',7,44,20]]);
session.lastView.generation = 42;
session.refreshCandidateView(); assert.deepEqual(calls.splice(0), [['view',7],['applyView','ok']]);
// 代次没变（选词后的 render 已经换上新代次）时收起不再重画，避免在 onChanged 里重入 render。
session.lastView.generation = 44;
session.refreshCandidateView(); assert.deepEqual(calls.splice(0), []);
console.log('候选适配器：跨页选择、过滤、管理、展开和收起刷新通过');
