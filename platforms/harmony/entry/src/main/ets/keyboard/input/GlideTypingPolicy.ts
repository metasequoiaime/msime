/**
 * Glide typing (滑行输入): when a touch on the letter keys becomes a glide, and the request (`msime_client_glide`) a finished stroke is sent to the Engine as.
 *
 * The host only decides that a touch was a glide and records where the finger went; which letters the stroke spells is the Engine's business. Everything here is plain arithmetic over rectangles and points, so the view hands it what ArkUI measured and draws nothing from it.
 */

/** One letter key's rectangle, in the coordinate space the touches are read in (vp). */
export interface GlideKeyRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** One touch sample: where the finger was and how long after touch-down, in milliseconds. */
export interface GlidePoint {
  x: number;
  y: number;
  t: number;
}

/** What decides whether a touch on the letter keys may become a glide at all. */
export interface GlideArming {
  /** The 滑行输入 setting. */
  enabled: boolean;
  /** The letters layer of the standard 26-key QWERTY keyboard is what is on screen: not Korean or Zhuyin keycaps, nine-key, handwriting, stroke, the symbol layer or a surface over the keys. */
  lettersLayer: boolean;
  /** `View.scheme` as the Engine last reported it. */
  engineScheme: number;
  /** Dedicated English. */
  english: boolean;
  /** `View.local_mode`. */
  localMode: string;
  /** Whether the focused editor composes through the Engine at all; a password field does not. */
  composes: boolean;
}

/** The request body `msime_client_glide` reads; the field names are its JSON keys. */
interface GlideRequest {
  keys: number[][];
  key_width: number;
  key_height: number;
  points: number[][];
}

const LETTERS: string = "abcdefghijklmnopqrstuvwxyz";

export class GlideTypingPolicy {
  /** The Engine's scheme number for quanpin, the only scheme glide decodes into. */
  static readonly QUANPIN_SCHEME: number = 0;
  /** How far, in key widths, the finger must have travelled sideways from touch-down before a touch on another letter counts as a glide. Horizontal only, so a vertical swipe on one key keeps meaning whatever it means there. */
  static readonly START_DISTANCE_KEYS: number = 0.4;
  /** The most points one request may carry, which is the C ABI's limit. */
  static readonly MAX_POINTS: number = 1024;
  /** The C ABI's limit on the request's size in bytes. */
  static readonly MAX_REQUEST_BYTES: number = 65536;
  /** How many samples the view keeps while the finger is down before thinning them; a stroke is never long enough to reach it at a normal touch rate. */
  static readonly BUFFER_POINTS: number = 4096;

  static armed(arming: GlideArming): boolean {
    return (
      arming.enabled &&
      arming.lettersLayer &&
      arming.composes &&
      arming.engineScheme === GlideTypingPolicy.QUANPIN_SCHEME &&
      !arming.english &&
      arming.localMode === "none"
    );
  }

  /** One slot per letter, a..z, none measured yet. */
  static emptyKeys(): (GlideKeyRect | null)[] {
    const keys: (GlideKeyRect | null)[] = [];
    for (let index: number = 0; index < LETTERS.length; index++) {
      keys.push(null);
    }
    return keys;
  }

  /** `a`..`z` as 0..25, anything else -1. */
  static letterIndex(letter: string): number {
    return letter.length === 1 ? LETTERS.indexOf(letter) : -1;
  }

  /**
   * The letter key under a point: the one whose rectangle, widened by `slackX` and `slackY` on each side (the half gaps a key's touch region reaches into), contains it, and the nearest centre where two widened rectangles overlap. -1 when no letter key is there.
   */
  static keyAt(
    keys: (GlideKeyRect | null)[],
    x: number,
    y: number,
    slackX: number,
    slackY: number,
  ): number {
    let found: number = -1;
    let nearest: number = Number.POSITIVE_INFINITY;
    for (let index: number = 0; index < keys.length; index++) {
      const key: GlideKeyRect | null = keys[index];
      if (key === null) {
        continue;
      }
      if (
        x < key.left - slackX ||
        x > key.left + key.width + slackX ||
        y < key.top - slackY ||
        y > key.top + key.height + slackY
      ) {
        continue;
      }
      const dx: number = x - (key.left + key.width / 2);
      const dy: number = y - (key.top + key.height / 2);
      const distance: number = dx * dx + dy * dy;
      if (distance < nearest) {
        nearest = distance;
        found = index;
      }
    }
    return found;
  }

  /** Whether a touch that went down on `downKey` has become a glide now that the finger is over `currentKey` at `x`. */
  static starts(
    downKey: number,
    currentKey: number,
    downX: number,
    x: number,
    keyWidth: number,
  ): boolean {
    return (
      downKey >= 0 &&
      currentKey >= 0 &&
      currentKey !== downKey &&
      keyWidth > 0 &&
      Math.abs(x - downX) >= GlideTypingPolicy.START_DISTANCE_KEYS * keyWidth
    );
  }

  /** The size of one letter key: the median over the keys, since the bottom row's letters share their row with Shift and Delete and need not be as wide as the others. 0 when no key has been measured. */
  static keyWidth(keys: (GlideKeyRect | null)[]): number {
    return GlideTypingPolicy.median(keys, true);
  }

  static keyHeight(keys: (GlideKeyRect | null)[]): number {
    return GlideTypingPolicy.median(keys, false);
  }

  /**
   * The time of the `index`th of `count` samples the system coalesced into one move event, spread evenly between the previous sample's time and the event's. The view times samples by the clock when each event arrives rather than by the platform's event timestamps, whose unit ArkUI does not declare.
   */
  static coalescedMillis(previous: number, now: number, index: number, count: number): number {
    const end: number = Math.max(previous, now);
    return previous + ((end - previous) * (index + 1)) / (count + 1);
  }

  /** At most `limit` of the points, evenly spaced along the stroke and always keeping the first and the last. */
  static downsample(points: GlidePoint[], limit: number): GlidePoint[] {
    if (points.length <= limit) {
      return points.slice();
    }
    if (limit < 2) {
      return points.slice(0, Math.max(0, limit));
    }
    const kept: GlidePoint[] = [];
    const step: number = (points.length - 1) / (limit - 1);
    for (let index: number = 0; index < limit; index++) {
      kept.push(points[Math.round(index * step)]);
    }
    return kept;
  }

  /**
   * The `msime_client_glide` request for a finished stroke, or null when there is nothing the Engine could read: a letter key not yet measured, fewer than two points, or a coordinate that is not a number.
   *
   * Coordinates are moved to the key area's own origin (the top-left of the letter keys) and rounded to a tenth of a vp, times to whole milliseconds and made non-decreasing. That keeps a 1024-point stroke well inside the request's byte limit.
   */
  static request(keys: (GlideKeyRect | null)[], points: GlidePoint[]): string | null {
    if (keys.length !== LETTERS.length) {
      return null;
    }
    let originX: number = Number.POSITIVE_INFINITY;
    let originY: number = Number.POSITIVE_INFINITY;
    for (const key of keys) {
      if (key === null || !GlideTypingPolicy.finiteRect(key)) {
        return null;
      }
      originX = Math.min(originX, key.left);
      originY = Math.min(originY, key.top);
    }
    const keyWidth: number = GlideTypingPolicy.keyWidth(keys);
    const keyHeight: number = GlideTypingPolicy.keyHeight(keys);
    const stroke: GlidePoint[] = GlideTypingPolicy.downsample(points, GlideTypingPolicy.MAX_POINTS);
    if (stroke.length < 2) {
      return null;
    }
    const centres: number[][] = [];
    for (const key of keys) {
      const rect: GlideKeyRect = key as GlideKeyRect;
      centres.push([
        GlideTypingPolicy.tenth(rect.left + rect.width / 2 - originX),
        GlideTypingPolicy.tenth(rect.top + rect.height / 2 - originY),
      ]);
    }
    const samples: number[][] = [];
    let time: number = 0;
    for (const point of stroke) {
      if (!Number.isFinite(point.x) || !Number.isFinite(point.y) || !Number.isFinite(point.t)) {
        return null;
      }
      time = Math.max(time, Math.round(point.t));
      samples.push([
        GlideTypingPolicy.tenth(point.x - originX),
        GlideTypingPolicy.tenth(point.y - originY),
        time,
      ]);
    }
    const request: GlideRequest = {
      keys: centres,
      key_width: GlideTypingPolicy.tenth(keyWidth),
      key_height: GlideTypingPolicy.tenth(keyHeight),
      points: samples,
    };
    const body: string = JSON.stringify(request);
    // Every character is ASCII, so the string's length is its UTF-8 length.
    return body.length <= GlideTypingPolicy.MAX_REQUEST_BYTES ? body : null;
  }

  private static finiteRect(key: GlideKeyRect): boolean {
    return (
      Number.isFinite(key.left) &&
      Number.isFinite(key.top) &&
      Number.isFinite(key.width) &&
      Number.isFinite(key.height) &&
      key.width > 0 &&
      key.height > 0
    );
  }

  private static median(keys: (GlideKeyRect | null)[], width: boolean): number {
    const sizes: number[] = [];
    for (const key of keys) {
      if (key !== null) {
        sizes.push(width ? key.width : key.height);
      }
    }
    if (sizes.length === 0) {
      return 0;
    }
    sizes.sort((a: number, b: number): number => a - b);
    return sizes[Math.floor(sizes.length / 2)];
  }

  private static tenth(value: number): number {
    return Math.round(value * 10) / 10;
  }
}
