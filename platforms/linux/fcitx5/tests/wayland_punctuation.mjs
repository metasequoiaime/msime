import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdirSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

// 仅在独立测试会话中运行；浏览器配置、输入和验证产物均不使用用户数据。
assert.equal(process.env.MSIME_ISOLATED_LINUX_TEST, '1');
const root = resolve('target/paired-punctuation-probe');
mkdirSync(root, {recursive: true});
writeFileSync(root + '/editor.html', `<title>MSIME synthetic punctuation test</title><textarea id="editor" autofocus></textarea><script>window.events=[];for(const type of ['keydown','keyup','input'])editor.addEventListener(type,e=>events.push({type,key:e.key,shift:e.shiftKey,value:editor.value,start:editor.selectionStart,end:editor.selectionEnd}));</script>`);
const url = pathToFileURL(root + '/editor.html').href;
const previous = execFileSync('fcitx5-remote', ['-n'], {encoding: 'utf8'}).trim();
const browser = spawn(process.env.MSIME_TEST_CHROME || 'google-chrome-stable', [
  '--user-data-dir=' + root + '/profile', '--remote-debugging-port=19329',
  '--no-first-run', '--no-default-browser-check', '--ozone-platform=wayland',
  '--enable-wayland-ime', url,
], {stdio: 'ignore'});
const sleep = ms => new Promise(r => setTimeout(r, ms));
let ws;
let heldKeys;
const results = [];
try {
  let page;
  for (let i = 0; i < 100 && !page; i++) {
    try {
      const pages = await (await fetch('http://127.0.0.1:19329/json')).json();
      page = pages.find(p => p.url === url);
    } catch {}
    if (!page) await sleep(100);
  }
  assert.ok(page, '独立 Chrome 未启动');
  ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise(r => ws.addEventListener('open', r, {once: true}));
  let id = 0;
  const pending = new Map();
  ws.addEventListener('message', e => {
    const msg = JSON.parse(e.data);
    if (pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
  });
  const send = (method, params = {}) => new Promise(r => {
    const n = ++id; pending.set(n, r); ws.send(JSON.stringify({id: n, method, params}));
  });
  const evaluate = async expression => (await send('Runtime.evaluate', {expression, returnByValue: true})).result.result.value;
  const snapshot = () => evaluate('({value:editor.value,start:editor.selectionStart,end:editor.selectionEnd,events})');
  await send('Page.bringToFront');
  await evaluate('editor.focus()');
  await sleep(700);
  execFileSync('fcitx5-remote', ['-s', 'msime']);
  execFileSync('fcitx5-remote', ['-o']);
  await sleep(1500);
  for (const [opening, closing, pair, shifted] of [
    // 既有映射把 > 对应到 〉，不是 》；书名号这里只验证补全，不扩展修复范围。
    ['less', null, '《》', true],
    ['parenleft', 'parenright', '（）', true],
    ['quotedbl', 'quotedbl', '“”', true],
    ['braceleft', 'braceright', '{}', true],
    ['bracketleft', 'bracketright', '【】', false],
  ]) {
    execFileSync('wtype', ['-k', 'Escape']);
    await evaluate('editor.value="";editor.setSelectionRange(0,0);events=[]');
    // 在同一次 wtype 调用中映射 Left/Right，供输入法解析合成方向键的键码。
    const stroke = key => ['-p', 'Left', '-p', 'Right', ...(shifted ? ['-M', 'shift', '-P', 'Shift_L'] : []),
      '-k', key, '-s', '100', ...(shifted ? ['-p', 'Shift_L', '-m', 'shift'] : [])];
    execFileSync('wtype', stroke(opening));
    await sleep(500);
    const paired = await snapshot();
    results.push({pair, paired});
    writeFileSync(root + '/result.json', JSON.stringify(results, null, 2));
    assert.equal(paired.value, pair, '未补出预期的标点对');
    assert.deepEqual([paired.start, paired.end], [1, 1], '闭标点被选中或光标未回移');
    assert.ok(paired.events.filter(e => e.key === 'ArrowLeft').every(e => !e.shift), '回移继承了 Shift');
    execFileSync('wtype', ['-d', '100', '-k', 'n', '-k', 'i', '-k', 'space']);
    await sleep(500);
    const typed = await snapshot();
    results.at(-1).typed = typed;
    assert.ok(typed.value.startsWith(pair[0]) && typed.value.endsWith(pair[1]), '输入覆盖了闭标点');
    assert.match(typed.value.slice(1, -1), /[\u4e00-\u9fff]/u, 'Shift 组合键误切换为英文');
    // 跳过闭标点的公共右移路径同样不能继承 Shift。
    if (!closing) continue;
    execFileSync('wtype', stroke(closing));
    await sleep(500);
    const closed = await snapshot();
    results.at(-1).closed = closed;
    writeFileSync(root + '/result.json', JSON.stringify(results, null, 2));
    assert.equal(closed.value, typed.value, '跳过闭标点时重复插入了标点');
    assert.deepEqual([closed.start, closed.end], [closed.value.length, closed.value.length], '右移产生了选区');
  }
  // 开、闭括号共用同一次 Shift，闭标点判断也要使用尚未落地的逻辑光标。
  await evaluate('editor.value="";editor.setSelectionRange(0,0);events=[]');
  execFileSync('wtype', ['-p', 'Left', '-p', 'Right', '-M', 'shift', '-P', 'Shift_L',
    '-k', 'parenleft', '-s', '100', '-k', 'parenright', '-p', 'Shift_L', '-m', 'shift']);
  await sleep(500);
  const held = await snapshot();
  results.push({held});
  writeFileSync(root + '/result.json', JSON.stringify(results, null, 2));
  assert.equal(held.value, '（）', '持续按住 Shift 时重复插入闭括号');
  assert.deepEqual([held.start, held.end], [2, 2], '开闭括号后光标位置不正确');

  // Shift 松开后不留空档，下一字母先到也必须先回移。
  await evaluate('editor.value="";editor.setSelectionRange(0,0);events=[]');
  execFileSync('wtype', ['-p', 'Left', '-p', 'Right', '-M', 'shift', '-P', 'Shift_L',
    '-k', 'less', '-p', 'Shift_L', '-m', 'shift', '-k', 'n', '-s', '100', '-k', 'i', '-k', 'space']);
  await sleep(500);
  const fast = await snapshot();
  results.push({fast});
  writeFileSync(root + '/result.json', JSON.stringify(results, null, 2));
  assert.ok(fast.value.startsWith('《') && fast.value.endsWith('》'), '快速输入覆盖闭标点或落到标点外');
  assert.match(fast.value.slice(1, -1), /[\u4e00-\u9fff]/u, '快速输入误切换为英文');
  assert.deepEqual([fast.start, fast.end], [fast.value.length - 1, fast.value.length - 1]);

  // 仍按住 Shift 时离开输入框，延后方向键不能落到另一个输入框。
  await evaluate('editor.value="";editor.setSelectionRange(0,0);events=[];window.other=document.createElement("textarea");document.body.append(other);other.addEventListener("keydown",e=>events.push({target:"other",key:e.key}))');
  heldKeys = spawn('wtype', ['-p', 'Left', '-p', 'Right', '-M', 'shift',
    '-k', 'less', '-s', '1500', '-m', 'shift']);
  const released = new Promise(r => heldKeys.once('close', r));
  await sleep(500);
  assert.equal((await snapshot()).value, '《》', '失焦用例未补全标点');
  await evaluate('other.focus()');
  assert.equal(await released, 0, '测试按键注入失败');
  heldKeys = undefined;
  await sleep(500);
  const blurred = await evaluate('({value:other.value,start:other.selectionStart,end:other.selectionEnd,events})');
  results.push({blurred});
  writeFileSync(root + '/result.json', JSON.stringify(results, null, 2));
  assert.equal(blurred.value, '');
  assert.ok(!blurred.events.some(e => e.target === 'other' && e.key.startsWith('Arrow')), '待转发方向键泄漏到新输入框');
  const screenshot = await send('Page.captureScreenshot');
  writeFileSync(root + '/editor.png', Buffer.from(screenshot.result.data, 'base64'));
  console.log('PASS: 五种标点补全、无修饰的左右移、持续 Shift、快速输入和失焦取消');
} finally {
  if (previous) execFileSync('fcitx5-remote', ['-s', previous]);
  heldKeys?.kill('SIGTERM');
  ws?.close();
  browser.kill('SIGTERM');
}
