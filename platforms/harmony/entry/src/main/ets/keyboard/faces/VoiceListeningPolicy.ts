import { TextPolicy } from "../TextPolicy";

/**
 * 手机聆听面板的排布、音量平滑、麦克风电平和中间文字裁剪，不依赖 ArkUI 或 NAPI，`tests/run.sh` 在 node 下直接测。Android 的 `VoiceListeningPolicy` 和 iOS 的 `KeyboardVoicePanelPolicy` 用同一组数值，三端面板长得一样、动得一样。
 *
 * 光圈以前是圆盘上的缩放变换，不占排布空间：放大到 1.5 倍时向上伸出 18vp，而面板内容本身就几乎和面板一样高，祖先节点都不裁剪，于是光圈画进了上面的工具条。现在圆盘连同光圈的最大外扩一起算进一个方框，方框放不下时先少留一行文字、再藏掉提示行，最后才缩小圆盘，满音量时光圈也在面板里面。
 */

/** 圆盘直径的上限与下限。 */
export const VOICE_ORB_MAX_VP: number = 64;
export const VOICE_ORB_MIN_VP: number = 40;
/** 光圈满音量时超出圆盘的宽度占直径的比例：64vp 的圆盘外扩 16vp，方框边长是直径的 1.5 倍。 */
export const VOICE_HALO_RATIO: number = 0.25;
/** 标题位要留两行时圆盘至少还有这么大，否则只留一行。 */
export const VOICE_TWO_LINE_MIN_ORB_VP: number = 52;
export const VOICE_PAD_TOP_VP: number = 4;
export const VOICE_GAP_INDICATOR_TITLE_VP: number = 6;
export const VOICE_GAP_TITLE_HINT_VP: number = 4;
export const VOICE_PAD_BOTTOM_VP: number = 8;
/** 安静时光圈也露出最大外扩的这一部分，看得出在听；开了「减弱动画」时光圈就停在这里。 */
export const VOICE_LEVEL_IDLE: number = 0.3;
/** 音量变大时跟上的时间常数，比回落快得多：一出声光圈就涨，停下来慢慢收。 */
export const VOICE_ATTACK_MS: number = 60;
export const VOICE_RELEASE_MS: number = 320;
/** 显示值离目标不到这么多就当已经到位，停掉逐帧重绘。 */
export const VOICE_LEVEL_SETTLE: number = 0.01;
/** 一帧最多按这么长推进，暂停很久后的第一帧不会一下跳到底。 */
export const VOICE_MAX_STEP_MS: number = 100;
export const VOICE_OUTER_RING_OPACITY: number = 0.16;
export const VOICE_INNER_RING_OPACITY: number = 0.26;
/** 电平送到界面的最短间隔，即不超过 10 Hz。 */
export const VOICE_LEVEL_INTERVAL_MS: number = 100;
export const VOICE_ELLIPSIS: string = "…";

// ---- 水杉娘指示区（#10313）----
// 非按住的会话把圆盘换成水杉娘、放大的耳部和随电平出现的红线。几何与 Android 的 `VoiceMascotDrawing`、iOS 的 `KeyboardVoicePanelPolicy` 相同，都画在 320×250 的视口里。

/** 人物用表情包里的这一张，与 Android、iOS 相同。 */
export const VOICE_MASCOT_FILE: string = "01.webp";
export const VOICE_MASCOT_VIEWPORT_WIDTH: number = 320;
export const VOICE_MASCOT_VIEWPORT_HEIGHT: number = 250;
/** 人物在视口里的方框：左上角 (20, 12)，边长 228。 */
export const VOICE_MASCOT_CHARACTER_X: number = 20;
export const VOICE_MASCOT_CHARACTER_Y: number = 12;
export const VOICE_MASCOT_CHARACTER_SIDE: number = 228;
/** 红线的圆心，在人物右耳。 */
export const VOICE_MASCOT_EAR_X: number = 218;
export const VOICE_MASCOT_EAR_Y: number = 111;
/** 红线从水平向右往上下各张开这么多度。 */
export const VOICE_MASCOT_ARC_SWEEP_DEGREES: number = 48;
export const VOICE_MASCOT_EAR_LINE_WIDTH: number = 2.5;
export const VOICE_MASCOT_EAR_FILL: string = "#FFDCC4";
export const VOICE_MASCOT_EAR_STROKE: string = "#9D5C4D";
export const VOICE_MASCOT_ARC_COLOR: string = "#D32F2F";
/** 放大的耳部轮廓（闭合）：起点 x、y，之后每六个数是一段三次贝塞尔的两个控制点和终点。 */
export const VOICE_MASCOT_EAR_OUTLINE: number[] = [
  187, 109, 189, 76, 235, 77, 234, 109, 235, 127, 215, 132, 213, 145, 207, 158, 186, 145, 190, 130,
];
/** 耳内的一笔（不闭合），格式同上。 */
export const VOICE_MASCOT_EAR_INNER: number[] = [
  199, 111, 198, 90, 224, 91, 222, 109, 222, 118, 205, 117, 206, 131,
];

/** 一道红线：半径和线宽是视口单位，`opacity` 是 0–1。 */
export interface VoiceMascotArc {
  radius: number;
  opacity: number;
  lineWidth: number;
}

/** 视口放进指示区时的缩放和左上角偏移，单位 vp。 */
export interface VoiceMascotFrame {
  scale: number;
  x: number;
  y: number;
}

/** 一次排布的结果，单位 vp。`orb` 是圆盘直径，`halo` 是光圈满音量时超出圆盘的宽度，`titleLines` 是标题位预留的行数，`showsHint` 为 false 时不画提示行，`top` 是整块内容居中后在区域上方让出的距离。 */
export interface VoiceListeningPlan {
  orb: number;
  halo: number;
  titleLines: number;
  showsHint: boolean;
  top: number;
}

/** 聆听面板的按钮只看会话的这几项。`heldByKey` 是会话由按住的键结束（空格长按或快捷键按住）；快捷键 LOCK 之后为 false，按点按开始的会话对待。 */
export interface VoiceListeningSession {
  busy: boolean;
  finishing: boolean;
  heldByKey: boolean;
  /** 结果刚上屏，面板正显示「识别完成」。 */
  done: boolean;
  /** 有一段可以显式插入的识别结果（输入位置变了时留下的）。 */
  hasText: boolean;
}

export class VoiceListeningPolicy {
  /**
   * 按钮行上面那块区域的排布：`height` 不含按钮行和它周围的边距，按住说话的会话没有按钮行时就是整个面板的高度；`titleLine`、`hintLine` 是标题和提示单行的实测高度。依次试「两行标题加提示」「一行标题加提示」「一行标题不带提示」，取第一个圆盘不小于下限的；两行时圆盘还要不小于 `VOICE_TWO_LINE_MIN_ORB_VP`。都放不下时圆盘先停在下限、光圈收窄，再不够才把圆盘缩到剩下的高度。
   *
   * 标题位总按预留的行数算，中间文字从一行换到两行时圆盘不跟着变；只占一行时空出来的那行落在提示下面。
   */
  static plan(height: number, titleLine: number, hintLine: number): VoiceListeningPlan {
    const available: number = Math.max(0, height);
    const lines: number[] = [2, 1, 1];
    const hints: boolean[] = [true, true, false];
    for (let index: number = 0; index < lines.length; index++) {
      const fixed: number = VoiceListeningPolicy.fixed(
        lines[index],
        hints[index],
        titleLine,
        hintLine,
      );
      const orb: number = Math.min(
        VOICE_ORB_MAX_VP,
        (available - fixed) / (1 + 2 * VOICE_HALO_RATIO),
      );
      if (lines[index] === 2 && orb < VOICE_TWO_LINE_MIN_ORB_VP) continue;
      if (orb >= VOICE_ORB_MIN_VP) {
        return VoiceListeningPolicy.placed(
          available,
          orb,
          orb * VOICE_HALO_RATIO,
          lines[index],
          hints[index],
          fixed,
        );
      }
    }
    const fixed: number = VoiceListeningPolicy.fixed(1, false, titleLine, hintLine);
    const box: number = Math.max(0, available - fixed);
    if (box >= VOICE_ORB_MIN_VP) {
      return VoiceListeningPolicy.placed(
        available,
        VOICE_ORB_MIN_VP,
        Math.min((box - VOICE_ORB_MIN_VP) / 2, VOICE_ORB_MIN_VP * VOICE_HALO_RATIO),
        1,
        false,
        fixed,
      );
    }
    return VoiceListeningPolicy.placed(available, box, 0, 1, false, fixed);
  }

  /** 方框之外的固定高度：上下内边距、方框与标题的间距、标题位，以及提示行和它上面的间距。 */
  static fixed(lines: number, hint: boolean, titleLine: number, hintLine: number): number {
    const value: number =
      VOICE_PAD_TOP_VP + VOICE_GAP_INDICATOR_TITLE_VP + lines * titleLine + VOICE_PAD_BOTTOM_VP;
    return hint ? value + VOICE_GAP_TITLE_HINT_VP + hintLine : value;
  }

  private static placed(
    height: number,
    orb: number,
    halo: number,
    lines: number,
    hint: boolean,
    fixed: number,
  ): VoiceListeningPlan {
    const used: number = orb + 2 * halo + fixed;
    return {
      orb: orb,
      halo: halo,
      titleLines: lines,
      showsHint: hint,
      top: Math.max(0, (height - used) / 2),
    };
  }

  /** 0–1，NaN 当作 0。 */
  static clamp(value: number): number {
    if (Number.isNaN(value)) return 0;
    return Math.max(0, Math.min(1, value));
  }

  /** 下一帧显示的音量：按经过的时间指数逼近目标，变大用 `VOICE_ATTACK_MS`，变小用 `VOICE_RELEASE_MS`；离目标不到 `VOICE_LEVEL_SETTLE` 时直接到位。 */
  static step(shown: number, target: number, elapsedMs: number): number {
    const goal: number = VoiceListeningPolicy.clamp(target);
    const elapsed: number = Number.isNaN(elapsedMs)
      ? 0
      : Math.max(0, Math.min(VOICE_MAX_STEP_MS, elapsedMs));
    const tau: number = goal > shown ? VOICE_ATTACK_MS : VOICE_RELEASE_MS;
    const next: number = shown + (goal - shown) * (1 - Math.exp(-elapsed / tau));
    return Math.abs(goal - next) < VOICE_LEVEL_SETTLE ? goal : next;
  }

  /** 显示值已经到位，不必再逐帧重绘。 */
  static settled(shown: number, target: number): boolean {
    return Math.abs(VoiceListeningPolicy.clamp(target) - shown) < VOICE_LEVEL_SETTLE;
  }

  /** 外圈光圈超出圆盘的宽度：安静时是满外扩的 `VOICE_LEVEL_IDLE`，满音量时是 `halo`；内圈取它的一半。 */
  static spread(shown: number, halo: number): number {
    return halo * (VOICE_LEVEL_IDLE + (1 - VOICE_LEVEL_IDLE) * VoiceListeningPolicy.clamp(shown));
  }

  /**
   * 光圈按方框画成满外扩的圆，再缩放到当前半径，缩放只是绘制变换，不重新排布。`share` 是这一圈占外圈外扩的比例：外圈 1，内圈 0.5。圆盘或方框为 0 时返回 0，什么也不画。
   */
  static ringScale(orb: number, halo: number, shown: number, share: number): number {
    const full: number = orb / 2 + halo;
    if (full <= 0) return 0;
    return (orb / 2 + share * VoiceListeningPolicy.spread(shown, halo)) / full;
  }

  /**
   * 一段 16 位小端单声道 PCM 的电平，0–1：均方根除以 32768，不超过 0.001 当作安静，其余按 -60 dB 到 0 dB 线性映射，与 Android 的算法一致。末尾不成对的半个样本不算。
   */
  static pcmLevel(data: ArrayBuffer): number {
    const samples: number = Math.floor(data.byteLength / 2);
    if (samples === 0) return 0;
    const view: DataView = new DataView(data);
    let sum: number = 0;
    for (let index: number = 0; index < samples; index++) {
      const sample: number = view.getInt16(index * 2, true);
      sum += sample * sample;
    }
    const rms: number = Math.sqrt(sum / samples) / 32768;
    if (rms <= 0.001) return 0;
    return VoiceListeningPolicy.clamp((20 * Math.log10(rms) + 60) / 60);
  }

  /**
   * 指示区画不画水杉娘：按住空格（`spaceHeld`）或按住快捷键（`heldByKey`，空格长按开始的会话同样记为按住，松手收尾时也不清）的会话保留圆盘和光圈，与 iOS 的长按空格、Android 的 `hold` 一致；点按开始与快捷键「锁定录音」之后（LOCK 清掉 `heldByKey`）画人物。
   */
  static showsMascot(spaceHeld: boolean, heldByKey: boolean): boolean {
    return !spaceHeld && !heldByKey;
  }

  /**
   * 按显示音量给出要画的红线，由内向外：音量不到 `VOICE_LEVEL_SETTLE` 时一道也不画（不沿用圆盘光圈的 `VOICE_LEVEL_IDLE`，安静时耳边没有线），第 i 道在音量达到 0.3 × i 时出现，半径 33 + 15i + 12 × 音量，不透明度 (90 + 165 × 音量) / 255，线宽 3 + 2 × 音量。
   */
  static mascotArcs(level: number): VoiceMascotArc[] {
    const value: number = VoiceListeningPolicy.clamp(level);
    const arcs: VoiceMascotArc[] = [];
    if (value < VOICE_LEVEL_SETTLE) return arcs;
    for (let index: number = 0; index < 3; index++) {
      if (value < 0.3 * index) continue;
      arcs.push({
        radius: 33 + 15 * index + 12 * value,
        opacity: (90 + 165 * value) / 255,
        lineWidth: 3 + 2 * value,
      });
    }
    return arcs;
  }

  /** 人物指示区的宽度：高度仍是方框边长 `box`（圆盘的 1.5 倍），宽度按视口比例放宽，不超过区域宽度。 */
  static mascotWidth(box: number, available: number): number {
    return Math.max(
      0,
      Math.min(available, (box * VOICE_MASCOT_VIEWPORT_WIDTH) / VOICE_MASCOT_VIEWPORT_HEIGHT),
    );
  }

  /** 视口等比缩放后居中放进 `width` × `height` 的指示区。 */
  static mascotFrame(width: number, height: number): VoiceMascotFrame {
    const scale: number = Math.max(
      0,
      Math.min(width / VOICE_MASCOT_VIEWPORT_WIDTH, height / VOICE_MASCOT_VIEWPORT_HEIGHT),
    );
    return {
      scale: scale,
      x: (width - VOICE_MASCOT_VIEWPORT_WIDTH * scale) / 2,
      y: (height - VOICE_MASCOT_VIEWPORT_HEIGHT * scale) / 2,
    };
  }

  /** 人物图片的解码边长：方框最大（圆盘上限的 1.5 倍）时人物的边长，只按这一个尺寸解码一次，排布变小时只缩放显示。 */
  static mascotDecodeSide(): number {
    return (
      (VOICE_ORB_MAX_VP * (1 + 2 * VOICE_HALO_RATIO) * VOICE_MASCOT_CHARACTER_SIDE) /
      VOICE_MASCOT_VIEWPORT_HEIGHT
    );
  }

  /** 中间文字去掉首尾空白，连续的空白（含换行）并成一个空格。 */
  static normalized(text: string): string {
    return text.replace(/\s+/gu, " ").trim();
  }

  /**
   * 放不下时从开头省略，留住最新说的那段：整段放得下就原样返回，否则返回「…」加上放得下的最长后缀；按码点切，不拆代理对。一个字也放不下时返回「…」加最后一个字。
   */
  static fitTail(text: string, fits: (candidate: string) => boolean): string {
    const value: string = VoiceListeningPolicy.normalized(text);
    if (value.length === 0 || fits(value)) return value;
    const points: string[] = TextPolicy.characters(value);
    let low: number = 1;
    let high: number = points.length - 1;
    let best: number = Math.max(0, points.length - 1);
    while (low <= high) {
      const middle: number = Math.floor((low + high) / 2);
      if (fits(VOICE_ELLIPSIS + points.slice(middle).join(""))) {
        best = middle;
        high = middle - 1;
      } else {
        low = middle + 1;
      }
    }
    return VOICE_ELLIPSIS + points.slice(best).join("");
  }

  /** 主按钮：录音中「说完了」，收尾时「正在识别…」（禁用），空闲时插入留下的结果或重新录音。按住说话的会话忙时和刚上屏时没有主按钮。 */
  static primaryLabel(session: VoiceListeningSession): string {
    if (session.done || (session.heldByKey && session.busy)) return "";
    if (session.finishing) return "正在识别…";
    if (session.busy) return "说完了";
    return session.hasText ? "插入文字" : "重新录音";
  }

  /**
   * 取消按钮：按住说话的会话忙时不给按钮，松手就是结束，结束和取消走按键本身；点按开始（含快捷键 LOCK 之后）的会话可以取消。不忙时（提示、出错、识别完成）一律留「返回键盘」，否则没有路离开面板。两个按钮都为空时面板不画按钮行。
   */
  static cancelLabel(session: VoiceListeningSession): string {
    if (!session.busy) return "返回键盘";
    return session.heldByKey ? "" : "取消";
  }
}

/** 把逐块算出的电平收成不超过 10 Hz 的更新：两次送出之间取这段时间里的最大值，短促的字音不会被漏掉。 */
export class VoiceLevelThrottle {
  private last: number = Number.NEGATIVE_INFINITY;
  private peak: number = 0;

  /** 收下一块的电平；到了送出的时候返回这段时间的最大值，否则返回 null。 */
  offer(level: number, nowMs: number): number | null {
    this.peak = Math.max(this.peak, VoiceListeningPolicy.clamp(level));
    if (nowMs - this.last < VOICE_LEVEL_INTERVAL_MS) return null;
    this.last = nowMs;
    const value: number = this.peak;
    this.peak = 0;
    return value;
  }
}
