// 执行实际 ArkTS 候选适配器，只替换 NAPI；校验算法由 Rust 行为测试覆盖。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {stripTypeScriptTypes} = require('node:module');
const filename = path.resolve(__dirname, '../entry/src/main/ets/keyboard/candidate/AiCandidateResponse.ets');
const source = fs.readFileSync(filename, 'utf8').replace(/^import .*;\r?\n/gm, '')
  .replace('export class ', 'class ');
const calls = [];
let response = {ok: true, value: ['我希望 AI 可以']};
const client = {aiCandidateProtocol(document) {
  calls.push(JSON.parse(document));
  return JSON.stringify(response);
}};
const Adapter = vm.runInNewContext(`${stripTypeScriptTypes(source)}; AiCandidateResponse;`, {client, JSON}, {filename});
const query = {identity: "0:wang'ai", generation: 17, session_id: 23, scheme: 0,
  pinyin_segments: ['wang', 'ai'], ai_eligible: true};
const body = JSON.stringify({choices: [{message: {content: JSON.stringify({candidates: [
  {text: '王 AI', segmentation: ['wang', 'ai']}
]})}}]});
assert.deepEqual(Adapter.parse(body, JSON.stringify(query), 3), ['我希望 AI 可以']);
assert.deepEqual(calls, [{op: 'parse', query, body, limit: 3}]);
response = {ok: false, error: 'synthetic_failure'};
assert.equal(Adapter.parse(body, JSON.stringify(query), 3), null);
response = {ok: true, value: null};
assert.equal(Adapter.parse(body, JSON.stringify(query), 3), null);
response = {ok: true, value: []};
assert.deepEqual(Adapter.parse(body, JSON.stringify(query), 3), []);
console.log('Harmony AI candidate adapter: original query and complete metadata reach shared validation');
