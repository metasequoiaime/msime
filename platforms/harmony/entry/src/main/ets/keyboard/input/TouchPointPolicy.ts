/**
 * 26 键双拼的按下点和键位几何（#10018）：宿主只负责把按下点和真实键位交给引擎，距离怎么算、哪个邻键更可能由 Rust 引擎决定（`msime_client_set_key_geometry` 与 `msime_client_touch_character`，见 crates/host-api/include/msime_client.h）。
 *
 * 这里只有矩形和数字的运算，视图把 ArkUI 量到的东西交进来，所以能在 node 下测。交给引擎的坐标一律是窗口坐标减去键盘根区域的原点。键位来自 `recordGlideKey` 记下的 `globalPosition`。按下点在按下那一刻只记成「落在被按的键里的哪个位置」（相对键的比例，`PendingTouches.down`），到点击时再按这个键当时的位置换回坐标（`PendingTouches.take`），所以按下点和几何总是同一帧的布局：按下和点击之间键盘重新排版（组字开始时工具条出现、提交后消失、改高度、转屏）时，按下点跟着键走，不会拿旧布局的点去比新布局的键。键的按下缩放是渲染变换，不改布局区域，键距的 `responseRegion` 也不挪动按下点，所以两边都是「没有经过键距挪动、不受按下缩放影响」的键区坐标；按在键距里的点比例会略小于 0 或大于 1，照样换算。
 */
import { GlideKeyRect } from "./GlideTypingPolicy";

/** 一次按下的位置，与几何同一坐标系（vp）。 */
export interface TouchPoint {
  x: number;
  y: number;
}

/** 视图交给 `KeyboardSession.pressTouch` 的一键：按下点和当时的键位布局（`geometryLayout`）。 */
export interface TouchKey {
  point: TouchPoint;
  layout: string;
}

/** 一键要不要带上按下点，由视图在按键那一刻填。 */
export interface TouchEligibility {
  /** 触屏键盘（`touchDesign()`）；2in1 的屏幕键盘不带。 */
  touchDesign: boolean;
  /** 屏幕上是标准 26 键的字母层，上面没有别的东西（`qwertyLettersShowing()`）：不是 14/18 键、九键、手写、笔画、注音、韩文、符号层或面板。 */
  lettersLayer: boolean;
  /** 引擎上次报告的 `View.scheme`。 */
  engineScheme: number;
  /** 专用英文。 */
  english: boolean;
  /** `View.local_mode`。 */
  localMode: string;
  /** 这一键以 Shift 送出（组字中的辅助码大写）。 */
  shifted: boolean;
}

/** 按下点落在被按的键里的位置：相对键左上角、以键宽和键高为单位。 */
interface PendingTouch {
  fx: number;
  fy: number;
  at: number;
}

const LETTERS: string = "abcdefghijklmnopqrstuvwxyz";

export class TouchPointPolicy {
  /** 引擎的双拼方案号（`SchemeTraits.SHUANGPIN`）。 */
  static readonly SHUANGPIN_SCHEME: number = 1;
  /** 按下点最多等点击多久（毫秒）。正常点击远在其内；长按满 500 ms 会变成角标提示，那一键本来就不输入字母。只防止没被点击取走的旧按下点（手指拖出键外才抬起）被之后的无障碍点击冒用。 */
  static readonly MAX_AGE_MS: number = 1000;
  /** 几何 JSON 的数值保留两位小数：ArkUI 量到的 vp 带浮点尾数，不截掉的话同一布局两次量出的字符串可能不同，白白换一次代次。 */
  static readonly PRECISION: number = 100;

  /** 只有 26 键双拼的字母键和 `;` 带按下点；辅助码 Shift、本地模式、专用英文、其他键面和 2in1 都照常走 `press`。 */
  static usesTouch(state: TouchEligibility): boolean {
    return (
      state.touchDesign &&
      state.lettersLayer &&
      state.engineScheme === TouchPointPolicy.SHUANGPIN_SCHEME &&
      !state.english &&
      state.localMode === "none" &&
      !state.shifted
    );
  }

  /** 会带按下点的键：小写字母和微软双拼的 `;`。 */
  static tracksKey(letter: string): boolean {
    return letter === ";" || (letter.length === 1 && LETTERS.indexOf(letter) >= 0);
  }

  /**
   * 几何 JSON 里除 `id` 以外的部分：`"keys":[[cx,cy,w,h] x 26，按 a..z],"semicolon":[cx,cy,w,h] 或 null,"duplicates":[]`。大屏分离式键盘右半边重复的 G、V 不单独上报：它们不记进键位（`KeyOptions.duplicate`），按下点按左边那个键上的同一相对位置换算，所以这里没有重复键。`semicolonShown` 是键面这一刻画不画微软双拼的 `;`，画着时 `semicolon` 必须已量到。有键还没量到、宽高不为正或数值不是有限数时返回 null，这时这一键不带按下点。每键都按当前键位现算，不缓存：26 个矩形每键拼一次的开销可以忽略，而缓存要靠视图在每处改动键位的地方记得作废，漏一处就会一直报旧的几何代次。布局没变时字符串不变，`KeyGeometryRegistry` 据此不重复下发。
   */
  static geometryLayout(
    keys: (GlideKeyRect | null)[],
    semicolonShown: boolean,
    semicolon: GlideKeyRect | null,
    originX: number,
    originY: number,
  ): string | null {
    if (keys.length !== LETTERS.length) {
      return null;
    }
    const rows: number[][] = [];
    for (const key of keys) {
      const row: number[] | null = TouchPointPolicy.keyRow(key, originX, originY);
      if (row === null) {
        return null;
      }
      rows.push(row);
    }
    let semicolonRow: number[] | null = null;
    if (semicolonShown) {
      semicolonRow = TouchPointPolicy.keyRow(semicolon, originX, originY);
      if (semicolonRow === null) {
        return null;
      }
    }
    return `"keys":${JSON.stringify(rows)},"semicolon":${JSON.stringify(semicolonRow)},"duplicates":[]`;
  }

  /** 一个带按下点的键现在的位置（窗口坐标）：字母取 `keys`（a..z），`;` 取 `semicolon`；没量到或不是这类键时为 null。 */
  static keyRect(
    keys: (GlideKeyRect | null)[],
    semicolon: GlideKeyRect | null,
    letter: string,
  ): GlideKeyRect | null {
    if (letter === ";") {
      return semicolon;
    }
    const index: number = letter.length === 1 ? LETTERS.indexOf(letter) : -1;
    return index < 0 || index >= keys.length ? null : keys[index];
  }

  /** 整份几何 JSON。 */
  static geometryJson(id: number, layout: string): string {
    return `{"id":${id},${layout}}`;
  }

  /** 按下点换到几何的坐标系；不是有限数时为 null。 */
  static point(
    windowX: number,
    windowY: number,
    originX: number,
    originY: number,
  ): TouchPoint | null {
    const x: number = windowX - originX;
    const y: number = windowY - originY;
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      return null;
    }
    return { x: x, y: y };
  }

  private static keyRow(
    key: GlideKeyRect | null,
    originX: number,
    originY: number,
  ): number[] | null {
    if (key === null) {
      return null;
    }
    const values: number[] = [
      key.left - originX + key.width / 2,
      key.top - originY + key.height / 2,
      key.width,
      key.height,
    ];
    for (const value of values) {
      if (!Number.isFinite(value)) {
        return null;
      }
    }
    if (key.width <= 0 || key.height <= 0) {
      return null;
    }
    return values.map((value: number): number => TouchPointPolicy.round(value));
  }

  private static round(value: number): number {
    return Math.round(value * TouchPointPolicy.PRECISION) / TouchPointPolicy.PRECISION;
  }
}

/**
 * 各键还没被点击取走的按下点。按下时记下，点击时取走并清掉，同一键下一次按下覆盖它，触摸被取消时丢掉。按键记而不按手指记：两指交替快打时，前一键的点击常在后一键按下之后才到。按下时只记相对被按的键的位置，点击时按这个键当时的位置换回键盘坐标，见文件头。
 */
export class PendingTouches {
  private readonly points: Map<string, PendingTouch> = new Map<string, PendingTouch>();

  /** `key` 是按下那一刻被按的键的区域（`event.target.area`，窗口坐标）；它宽高不为正或数值不是有限数时这一键不记按下点。 */
  down(letter: string, windowX: number, windowY: number, key: GlideKeyRect, at: number): void {
    const fx: number = (windowX - key.left) / key.width;
    const fy: number = (windowY - key.top) / key.height;
    if (!(key.width > 0) || !(key.height > 0) || !Number.isFinite(fx) || !Number.isFinite(fy)) {
      this.points.delete(letter);
      return;
    }
    this.points.set(letter, { fx: fx, fy: fy, at: at });
  }

  cancel(letter: string): void {
    this.points.delete(letter);
  }

  /**
   * 取走这一键的按下点，换成键盘坐标（与 `geometryLayout` 同一坐标系）：`key` 是这个键现在的位置（`TouchPointPolicy.keyRect`），`originX/originY` 是键盘根区域现在的原点。没有、已超过 `MAX_AGE_MS`（含时钟回拨）、键现在没量到或宽高不为正时为 null。不论结果都清掉。
   */
  take(
    letter: string,
    now: number,
    key: GlideKeyRect | null,
    originX: number,
    originY: number,
  ): TouchPoint | null {
    const pending: PendingTouch | undefined = this.points.get(letter);
    this.points.delete(letter);
    if (pending === undefined || key === null || !(key.width > 0) || !(key.height > 0)) {
      return null;
    }
    const age: number = now - pending.at;
    if (age < 0 || age > TouchPointPolicy.MAX_AGE_MS) {
      return null;
    }
    return TouchPointPolicy.point(
      key.left + pending.fx * key.width,
      key.top + pending.fy * key.height,
      originX,
      originY,
    );
  }

  /** 键面被替换时全部丢掉。 */
  clear(): void {
    this.points.clear();
  }
}

/**
 * 当前会话注册过的几何。只在布局变了或换了会话时才下发，其余按键直接用已注册的代次；下发失败时这一键不带按下点，下一键再试。代次在本进程内递增，从 1 开始，不会是 0（0 表示清除）。
 */
export class KeyGeometryRegistry {
  private handle: number = 0;
  private layout: string | null = null;
  private id: number = 0;
  private lastId: number = 0;

  /** 这一键要用的几何代次，0 表示不带按下点。`send` 下发整份 JSON 并报告引擎是否接受。 */
  register(handle: number, layout: string, send: (json: string) => boolean): number {
    if (handle === 0) {
      return 0;
    }
    if (handle === this.handle && layout === this.layout && this.id !== 0) {
      return this.id;
    }
    this.lastId = this.lastId >= 0xffffffff ? 1 : this.lastId + 1;
    const id: number = this.lastId;
    if (!send(TouchPointPolicy.geometryJson(id, layout))) {
      this.reset();
      return 0;
    }
    this.handle = handle;
    this.layout = layout;
    this.id = id;
    return id;
  }

  /** 会话销毁或重建时调用：新会话里没有几何，下一键要重新下发。 */
  reset(): void {
    this.handle = 0;
    this.layout = null;
    this.id = 0;
  }
}
