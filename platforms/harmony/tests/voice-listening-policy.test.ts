import {
  VOICE_HALO_RATIO,
  VOICE_MASCOT_EAR_INNER,
  VOICE_MASCOT_EAR_OUTLINE,
  VOICE_MASCOT_FILE,
  VOICE_ORB_MAX_VP,
  VOICE_ORB_MIN_VP,
  VOICE_TWO_LINE_MIN_ORB_VP,
  VoiceLevelThrottle,
  VoiceListeningPlan,
  VoiceListeningPolicy,
  VoiceListeningSession,
} from "../entry/src/main/ets/keyboard/faces/VoiceListeningPolicy";

declare const process: { exitCode?: number };
let failures = 0;
let checked = 0;
function expect(name: string, condition: boolean, detail: string = ""): void {
  checked++;
  if (!condition) {
    failures++;
    console.error(`FAIL voice listening ${name}${detail.length > 0 ? `: ${detail}` : ""}`);
  }
}
function near(actual: number, expected: number, tolerance: number = 1e-6): boolean {
  return Math.abs(actual - expected) <= tolerance;
}
function describe(plan: VoiceListeningPlan): string {
  return JSON.stringify(plan);
}
function used(plan: VoiceListeningPlan, titleLine: number, hintLine: number): number {
  return (
    plan.orb +
    2 * plan.halo +
    VoiceListeningPolicy.fixed(plan.titleLines, plan.showsHint, titleLine, hintLine)
  );
}

// ---- plan ----
const TITLE = 22;
const HINT = 18;
{
  // 很高的区域：两行标题加提示，圆盘取上限 64vp，光圈外扩 16vp，内容在区域里居中。
  const plan = VoiceListeningPolicy.plan(300, TITLE, HINT);
  expect(
    "plan/tall",
    VOICE_ORB_MAX_VP === 64 &&
      plan.orb === VOICE_ORB_MAX_VP &&
      near(plan.halo, 16) &&
      plan.titleLines === 2 &&
      plan.showsHint &&
      near(plan.top, (300 - (64 + 32 + 84)) / 2),
    describe(plan),
  );
  const huge = VoiceListeningPolicy.plan(1000, 40, 32);
  expect(
    "plan/cap",
    huge.orb === 64 && near(huge.halo, 16) && huge.titleLines === 2,
    describe(huge),
  );
}
{
  // 按住说话的会话收尾时没有按钮行，区域就是整个面板（`surfaceHeight`：4 行 44vp 加高度调整与 3 道行距）。最矮的面板（高度调整 -12、行距 4vp）是 176vp，仍是两行标题加提示，圆盘约 61vp；出厂面板 197vp 时圆盘到上限 64vp。
  const smallest = 44 * 4 - 12 + 3 * 4;
  const held = VoiceListeningPolicy.plan(smallest, TITLE, HINT);
  expect(
    "plan/held-full-height",
    held.titleLines === 2 &&
      held.showsHint &&
      near(held.orb, (smallest - 84) / 1.5) &&
      held.orb > 58 &&
      held.orb < 64,
    describe(held),
  );
  const factory = 44 * 4 + 3 * 7;
  const heldFactory = VoiceListeningPolicy.plan(factory, TITLE, HINT);
  expect(
    "plan/held-factory",
    heldFactory.titleLines === 2 && heldFactory.showsHint && heldFactory.orb === 64,
    describe(heldFactory),
  );
  // 点按开始的会话要让出按钮行（约 40vp）和它下面的 10vp：同一块出厂面板只剩一行标题。
  const tapped = VoiceListeningPolicy.plan(factory - 40 - 10, TITLE, HINT);
  expect(
    "plan/tap-start-with-actions",
    tapped.titleLines === 1 && tapped.showsHint && tapped.orb < 64,
    describe(tapped),
  );
}
{
  // 两行时圆盘不足 52vp，退到一行标题加提示；这个高度下一行时的圆盘还在上限以内。
  const height = 84 + 1.5 * 45;
  const plan = VoiceListeningPolicy.plan(height, TITLE, HINT);
  expect(
    "plan/one-line",
    plan.titleLines === 1 &&
      plan.showsHint &&
      near(plan.orb, (height - 62) / 1.5) &&
      near(plan.halo, plan.orb * VOICE_HALO_RATIO),
    describe(plan),
  );
}
{
  // 两行时刚好 52vp，仍留两行。
  const height = 84 + 1.5 * VOICE_TWO_LINE_MIN_ORB_VP;
  const plan = VoiceListeningPolicy.plan(height, TITLE, HINT);
  expect(
    "plan/two-line-threshold",
    plan.titleLines === 2 && near(plan.orb, VOICE_TWO_LINE_MIN_ORB_VP),
    describe(plan),
  );
}
{
  // 带提示时圆盘不足 40vp，藏掉提示行。
  const height = 62 + 1.5 * 39;
  const plan = VoiceListeningPolicy.plan(height, TITLE, HINT);
  expect(
    "plan/no-hint",
    plan.titleLines === 1 && !plan.showsHint && near(plan.orb, (height - 40) / 1.5),
    describe(plan),
  );
}
{
  // 最后一档：圆盘停在下限，光圈收窄到剩下的空间。
  const plan = VoiceListeningPolicy.plan(99, TITLE, HINT);
  expect(
    "plan/min-orb",
    plan.orb === VOICE_ORB_MIN_VP && near(plan.halo, 9.5) && !plan.showsHint && near(plan.top, 0),
    describe(plan),
  );
}
{
  // 连下限都放不下：圆盘缩到剩下的高度，不画光圈。
  const plan = VoiceListeningPolicy.plan(60, TITLE, HINT);
  expect(
    "plan/squeezed",
    near(plan.orb, 20) && plan.halo === 0 && near(plan.top, 0),
    describe(plan),
  );
  const none = VoiceListeningPolicy.plan(-10, TITLE, HINT);
  expect("plan/negative", none.orb === 0 && none.halo === 0 && none.top === 0, describe(none));
  const zero = VoiceListeningPolicy.plan(0, 0, 0);
  expect("plan/unmeasured", zero.orb === 0 && zero.halo === 0, describe(zero));
}
// 不变量：光圈满外扩时也在区域里，光圈不超过直径的四分之一，圆盘不超过上限；字体放大时同样成立。
for (const [titleLine, hintLine] of [
  [22, 18],
  [30, 24],
  [44, 36],
  [0, 0],
]) {
  for (let height = 0; height <= 480; height += 0.5) {
    const plan = VoiceListeningPolicy.plan(height, titleLine, hintLine);
    const fixedMin = VoiceListeningPolicy.fixed(1, false, titleLine, hintLine);
    const fits =
      height < fixedMin ? plan.orb === 0 : used(plan, titleLine, hintLine) <= height + 1e-9;
    if (
      !fits ||
      plan.orb > VOICE_ORB_MAX_VP ||
      plan.halo > plan.orb * VOICE_HALO_RATIO + 1e-9 ||
      plan.top < 0 ||
      near(
        plan.top * 2 + used(plan, titleLine, hintLine),
        Math.max(height, used(plan, titleLine, hintLine)),
      ) === false
    ) {
      expect(`plan/invariant ${height} ${titleLine}/${hintLine}`, false, describe(plan));
      break;
    }
  }
}
expect("plan/invariants", true);

// ---- step / settled ----
expect("step/attack", near(VoiceListeningPolicy.step(0, 1, 60), 1 - Math.exp(-1)));
// 一帧最多推进 100ms，所以回落按 100ms 而不是一个完整的时间常数验。
expect("step/release", near(VoiceListeningPolicy.step(1, 0, 100), Math.exp(-100 / 320)));
expect(
  "step/elapsed-capped",
  VoiceListeningPolicy.step(0, 1, 1000) === VoiceListeningPolicy.step(0, 1, 100),
);
expect("step/negative-elapsed", VoiceListeningPolicy.step(0.4, 1, -5) === 0.4);
expect("step/nan-elapsed", VoiceListeningPolicy.step(0.4, 1, Number.NaN) === 0.4);
expect(
  "step/target-clamped",
  VoiceListeningPolicy.step(0, 5, 50) === VoiceListeningPolicy.step(0, 1, 50),
);
expect(
  "step/nan-target-is-quiet",
  VoiceListeningPolicy.step(0.5, Number.NaN, 50) === VoiceListeningPolicy.step(0.5, 0, 50),
);
expect("step/snaps", VoiceListeningPolicy.step(0.995, 1, 1) === 1);
{
  // 逐帧推进最终到位，到位后 settled 为真，不再需要帧回调。
  let shown = 0;
  let frames = 0;
  while (!VoiceListeningPolicy.settled(shown, 0.8) && frames < 1000) {
    shown = VoiceListeningPolicy.step(shown, 0.8, 16);
    frames++;
  }
  expect("step/converges-rising", shown === 0.8 && frames < 30, `${shown} after ${frames}`);
  frames = 0;
  while (!VoiceListeningPolicy.settled(shown, 0) && frames < 1000) {
    shown = VoiceListeningPolicy.step(shown, 0, 16);
    frames++;
  }
  expect(
    "step/converges-falling",
    shown === 0 && frames > 30 && frames < 200,
    `${shown} after ${frames}`,
  );
}
expect("settled/near", VoiceListeningPolicy.settled(0.995, 1));
expect("settled/far", !VoiceListeningPolicy.settled(0.5, 1));
expect("settled/nan", VoiceListeningPolicy.settled(0, Number.NaN));

// ---- rings ----
expect("ring/outer-full", near(VoiceListeningPolicy.ringScale(72, 18, 1, 1), 1));
expect("ring/outer-idle", near(VoiceListeningPolicy.ringScale(72, 18, 0, 1), (36 + 18 * 0.3) / 54));
expect("ring/inner-full", near(VoiceListeningPolicy.ringScale(72, 18, 1, 0.5), (36 + 9) / 54));
expect("ring/over-full-clamped", near(VoiceListeningPolicy.ringScale(72, 18, 3, 1), 1));
expect("ring/empty", VoiceListeningPolicy.ringScale(0, 0, 1, 1) === 0);
expect("ring/no-halo", near(VoiceListeningPolicy.ringScale(40, 0, 1, 1), 1));

// ---- pcmLevel ----
function pcm(samples: number[], extraByte: boolean = false): ArrayBuffer {
  const buffer = new ArrayBuffer(samples.length * 2 + (extraByte ? 1 : 0));
  const view = new DataView(buffer);
  samples.forEach((sample: number, index: number): void => view.setInt16(index * 2, sample, true));
  if (extraByte) view.setUint8(samples.length * 2, 0x7f);
  return buffer;
}
function square(amplitude: number, count: number = 320): number[] {
  return Array.from({ length: count }, (_: unknown, index: number): number =>
    index % 2 === 0 ? amplitude : -amplitude,
  );
}
expect("pcm/empty", VoiceListeningPolicy.pcmLevel(new ArrayBuffer(0)) === 0);
expect("pcm/one-byte", VoiceListeningPolicy.pcmLevel(new ArrayBuffer(1)) === 0);
expect("pcm/silence", VoiceListeningPolicy.pcmLevel(pcm(square(0))) === 0);
expect("pcm/below-floor", VoiceListeningPolicy.pcmLevel(pcm(square(30))) === 0);
expect("pcm/full-scale", near(VoiceListeningPolicy.pcmLevel(pcm(square(32767))), 1, 1e-4));
expect(
  "pcm/minus-40db",
  near(VoiceListeningPolicy.pcmLevel(pcm(square(328))), (20 * Math.log10(328 / 32768) + 60) / 60),
);
expect("pcm/never-above-one", VoiceListeningPolicy.pcmLevel(pcm(square(-32768))) <= 1);
{
  // 小端：字节 00 40 是 0x4000（16384），均方根 0.5，约 -6 dB。
  const bytes = new Uint8Array([0x00, 0x40, 0x00, 0x40]);
  expect(
    "pcm/little-endian",
    near(VoiceListeningPolicy.pcmLevel(bytes.buffer), (20 * Math.log10(0.5) + 60) / 60),
  );
}
expect(
  "pcm/odd-trailing-byte",
  VoiceListeningPolicy.pcmLevel(pcm(square(3277), true)) ===
    VoiceListeningPolicy.pcmLevel(pcm(square(3277))),
);

// ---- normalized / fitTail ----
expect(
  "normalized/collapse",
  VoiceListeningPolicy.normalized("  合成\n\n 文字\t甲　乙  ") === "合成 文字 甲 乙",
);
expect("normalized/blank", VoiceListeningPolicy.normalized(" \n\t ") === "");
function codePoints(limit: number): (candidate: string) => boolean {
  return (candidate: string): boolean => Array.from(candidate).length <= limit;
}
expect("fit/whole", VoiceListeningPolicy.fitTail(" 合成 \n 文字 ", codePoints(10)) === "合成 文字");
expect("fit/empty", VoiceListeningPolicy.fitTail("   ", codePoints(0)) === "");
expect("fit/head-ellipsis", VoiceListeningPolicy.fitTail("abcdefghij", codePoints(5)) === "…ghij");
{
  const emoji = "\u{1F600}";
  const fitted = VoiceListeningPolicy.fitTail(`${emoji}${emoji}${emoji}${emoji}`, codePoints(3));
  expect("fit/surrogates", fitted === `…${emoji}${emoji}`, fitted);
  expect(
    "fit/no-lone-surrogate",
    !/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/.test(fitted),
  );
  const nothing = VoiceListeningPolicy.fitTail(`甲乙${emoji}`, (): boolean => false);
  expect("fit/nothing-fits", nothing === `…${emoji}`, nothing);
}
{
  // 结果是放得下的最长后缀：再多一个字就放不下。
  const text = "合成的一段比较长的中间识别文字用来检查二分查找";
  for (let limit = 2; limit < Array.from(text).length; limit++) {
    const fits = codePoints(limit);
    const fitted = VoiceListeningPolicy.fitTail(text, fits);
    const tail = fitted.slice(1);
    const longer =
      "…" +
      Array.from(text)
        .slice(Array.from(text).length - Array.from(tail).length - 1)
        .join("");
    if (!fits(fitted) || fits(longer) || !text.endsWith(tail)) {
      expect(`fit/longest-suffix ${limit}`, false, fitted);
    }
  }
  expect("fit/longest-suffix", true);
}
{
  // 宽度按字形计的判定（模拟实测）：中文按 2 个单位，其余按 1 个。
  const width = (candidate: string): number =>
    Array.from(candidate).reduce(
      (sum: number, point: string): number => sum + (/[一-鿿]/.test(point) ? 2 : 1),
      0,
    );
  const fitted = VoiceListeningPolicy.fitTail(
    "hello 合成文字",
    (candidate: string): boolean => width(candidate) <= 7,
  );
  expect("fit/measured", fitted === "…成文字", fitted);
}

// ---- buttons ----
{
  const session = (overrides: Partial<VoiceListeningSession>): VoiceListeningSession => ({
    busy: false,
    finishing: false,
    heldByKey: false,
    done: false,
    hasText: false,
    ...overrides,
  });
  const labels = (state: VoiceListeningSession): string =>
    `${VoiceListeningPolicy.primaryLabel(state)}|${VoiceListeningPolicy.cancelLabel(state)}`;
  // 按住说话（空格长按、快捷键按住）录音和收尾时都没有按钮，面板不画按钮行。
  expect("buttons/held-recording", labels(session({ busy: true, heldByKey: true })) === "|");
  expect(
    "buttons/held-finishing",
    labels(session({ busy: true, finishing: true, heldByKey: true })) === "|",
  );
  // 快捷键 LOCK 之后 heldByKey 为 false：和点按开始的会话一样给「说完了」「取消」。
  const tapped = labels(session({ busy: true }));
  expect("buttons/tap-recording", tapped === "说完了|取消", tapped);
  expect("buttons/locked-is-tap", labels(session({ busy: true, heldByKey: false })) === tapped);
  expect(
    "buttons/tap-finishing",
    labels(session({ busy: true, finishing: true })) === "正在识别…|取消",
  );
  // 不忙时一律能「返回键盘」，按住说话结束后也一样。
  expect("buttons/idle", labels(session({})) === "重新录音|返回键盘");
  expect("buttons/idle-after-hold", labels(session({ heldByKey: true })) === "重新录音|返回键盘");
  expect("buttons/idle-with-text", labels(session({ hasText: true })) === "插入文字|返回键盘");
  expect("buttons/done", labels(session({ done: true, heldByKey: true })) === "|返回键盘");
}

// ---- mascot ----
{
  // 只有两个按住标志都为 false 的会话画人物：点按开始，或快捷键 LOCK 之后；按住空格、按住快捷键保留圆盘。
  expect("mascot/tap", VoiceListeningPolicy.showsMascot(false, false));
  expect("mascot/space-held", !VoiceListeningPolicy.showsMascot(true, true));
  // 空格松手收尾时 `spaceVoiceHeld` 已清，`heldByKey` 仍为 true；按住快捷键也是这个组合。
  expect("mascot/released-or-hotkey", !VoiceListeningPolicy.showsMascot(false, true));
  expect("mascot/space-only", !VoiceListeningPolicy.showsMascot(true, false));
  expect("mascot/file", VOICE_MASCOT_FILE === "01.webp");

  // 安静时一道红线也没有，不沿用光圈的 0.3 静息外扩。
  expect("mascot/arcs-silent", VoiceListeningPolicy.mascotArcs(0).length === 0);
  expect("mascot/arcs-near-silent", VoiceListeningPolicy.mascotArcs(0.005).length === 0);
  expect("mascot/arcs-nan", VoiceListeningPolicy.mascotArcs(Number.NaN).length === 0);
  expect("mascot/arcs-negative", VoiceListeningPolicy.mascotArcs(-1).length === 0);
  expect("mascot/arcs-one", VoiceListeningPolicy.mascotArcs(0.1).length === 1);
  expect("mascot/arcs-one-below", VoiceListeningPolicy.mascotArcs(0.29).length === 1);
  expect("mascot/arcs-two", VoiceListeningPolicy.mascotArcs(0.3).length === 2);
  expect("mascot/arcs-three", VoiceListeningPolicy.mascotArcs(0.6).length === 3);
  const full = VoiceListeningPolicy.mascotArcs(1);
  expect(
    "mascot/arcs-full",
    JSON.stringify(full.map((arc) => [arc.radius, arc.lineWidth])) === "[[45,5],[60,5],[75,5]]",
    JSON.stringify(full),
  );
  expect("mascot/arcs-full-opaque", near(full[0].opacity, 1));
  expect(
    "mascot/arcs-clamped",
    JSON.stringify(VoiceListeningPolicy.mascotArcs(3)) === JSON.stringify(full),
  );
  const quiet = VoiceListeningPolicy.mascotArcs(0.1)[0];
  expect("mascot/arc-radius", near(quiet.radius, 34.2));
  expect("mascot/arc-opacity", near(quiet.opacity, 106.5 / 255));
  expect("mascot/arc-width", near(quiet.lineWidth, 3.2));

  // 人物指示区：高度是方框边长，宽度按 320×250 放宽、不超过区域；视口等比缩放后居中。
  const box = VOICE_ORB_MAX_VP * (1 + 2 * VOICE_HALO_RATIO);
  expect("mascot/width", near(VoiceListeningPolicy.mascotWidth(box, 400), 122.88));
  expect("mascot/width-narrow", near(VoiceListeningPolicy.mascotWidth(box, 100), 100));
  expect("mascot/width-zero", VoiceListeningPolicy.mascotWidth(0, 400) === 0);
  const wide = VoiceListeningPolicy.mascotFrame(122.88, box);
  expect(
    "mascot/frame-wide",
    near(wide.scale, box / 250) && near(wide.x, 0) && near(wide.y, 0),
    JSON.stringify(wide),
  );
  const narrow = VoiceListeningPolicy.mascotFrame(100, box);
  expect(
    "mascot/frame-narrow",
    near(narrow.scale, 100 / 320) &&
      near(narrow.x, 0) &&
      near(narrow.y, (box - (250 * 100) / 320) / 2),
    JSON.stringify(narrow),
  );
  const empty = VoiceListeningPolicy.mascotFrame(0, 0);
  expect(
    "mascot/frame-empty",
    empty.scale === 0 && empty.x === 0 && empty.y === 0,
    JSON.stringify(empty),
  );
  expect("mascot/decode-side", near(VoiceListeningPolicy.mascotDecodeSide(), (96 * 228) / 250));

  // 耳部路径是起点加整段的三次贝塞尔：与 Android `VoiceMascotDrawing` 一样，轮廓三段、耳内两段。
  expect("mascot/ear-outline", VOICE_MASCOT_EAR_OUTLINE.length === 2 + 3 * 6);
  expect("mascot/ear-inner", VOICE_MASCOT_EAR_INNER.length === 2 + 2 * 6);
}

// ---- throttle ----
{
  const throttle = new VoiceLevelThrottle();
  const results = [
    throttle.offer(0.2, 0),
    throttle.offer(0.8, 50),
    throttle.offer(0.1, 99),
    throttle.offer(0.3, 100),
    throttle.offer(0.1, 150),
    throttle.offer(0.05, 250),
    throttle.offer(Number.NaN, 400),
    throttle.offer(7, 500),
  ];
  const expected = [0.2, null, null, 0.8, null, 0.1, 0, 1];
  expect(
    "throttle/peak-hold",
    JSON.stringify(results) === JSON.stringify(expected),
    JSON.stringify(results),
  );
}

if (failures > 0) {
  process.exitCode = 1;
} else {
  console.log(`voice listening policy: ${checked} checks passed`);
}
