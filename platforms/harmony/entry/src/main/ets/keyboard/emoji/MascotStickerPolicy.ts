import { TextPolicy } from "../TextPolicy";
import { utf8Length } from "../Utf8";

/**
 * 表情面板里的水杉娘表情包：清单解析与边界校验、页签顺序与默认页、网格列数和复制后的提示文案。
 *
 * 素材是仓库里的 `resources/stickers/shuishan`（66 张 320px 透明 WebP 加 `manifest.json`），由 `stage-resources.sh` 放进 resfile，安装后在 `context.resourceDir/stickers/shuishan`；iOS 与 Android 读的是同一份。清单顺序就是显示顺序，`name` 是这张的含义，读屏标签和提示都用它。上限与 iOS 的 `KeyboardStickerCatalog` 相同。
 */

/** 表情包在 resfile 里的相对目录。 */
export const MASCOT_STICKER_DIRECTORY: string = "stickers/shuishan";
export const MASCOT_STICKER_MANIFEST: string = "manifest.json";
export const MASCOT_STICKER_PACK_NAME: string = "水杉娘";
/** 表情面板里「水杉娘」这一页的标识，页签顺序和打开时停在哪一页都按它认。 */
export const MASCOT_STICKER_PANEL: string = "mascot-stickers";
export const MASCOT_STICKER_MAX_COUNT: number = 256;
export const MASCOT_STICKER_MAX_NAME_CHARACTERS: number = 16;
export const MASCOT_STICKER_MAX_MANIFEST_BYTES: number = 64 * 1024;
/** 单张贴纸文件的上限；随包的都在 40KB 以内。 */
export const MASCOT_STICKER_MAX_IMAGE_BYTES: number = 512 * 1024;
/** 手机竖屏一行至少五个；更宽的面板按每格约 72vp 增加列数。 */
export const MASCOT_STICKER_MIN_COLUMNS: number = 5;
export const MASCOT_STICKER_CELL_VP: number = 72;
/** 每格图片四周的留白。 */
export const MASCOT_STICKER_INSET_VP: number = 4;

/** 剪贴板服务的两种已知拒绝（`@ohos.pasteboard` 的 `setData`）：另一次复制或粘贴正在进行，以及系统禁止复制。 */
export const PASTEBOARD_BUSY_CODE: number = 27787277;
export const PASTEBOARD_PROHIBITED_CODE: number = 27787278;

const FILE_PATTERN: RegExp = /^[0-9]{2}\.webp$/;

export interface MascotSticker {
  /** 清单里的文件名，例如 `01.webp`，只会是两位数字加 `.webp`。 */
  readonly file: string;
  /** 这张的含义，例如「开心」。 */
  readonly name: string;
}

/** 复制没成功的原因，各对应一句给用户看的提示。 */
export enum MascotStickerFailure {
  /** 图片文件读不出来或解码失败。 */
  IMAGE_UNREADABLE,
  /** 剪贴板正被另一次复制或粘贴占用。 */
  CLIPBOARD_BUSY,
  /** 系统不允许写剪贴板。 */
  CLIPBOARD_PROHIBITED,
  /** 其他写剪贴板的失败。 */
  CLIPBOARD_FAILED,
}

export class MascotStickerPolicy {
  /**
   * 读清单并逐条校验：根是对象，`stickers` 是非空数组且不超过上限；每条的文件名只能是两位数字加 `.webp` 且不重复，名字去掉首尾空白后非空、不超过 16 个字符。有任何一条不合格就整份不用，返回空表，面板也就不显示这个页签。
   */
  static parse(document: string | null): MascotSticker[] {
    if (document === null || document.length === 0 || utf8Length(document) > MASCOT_STICKER_MAX_MANIFEST_BYTES) {
      return [];
    }
    let decoded: unknown;
    try {
      decoded = JSON.parse(document);
    } catch {
      return [];
    }
    if (decoded === null || typeof decoded !== "object" || Array.isArray(decoded)) {
      return [];
    }
    const entries: unknown = (decoded as { stickers?: unknown }).stickers;
    if (!Array.isArray(entries) || entries.length === 0 || entries.length > MASCOT_STICKER_MAX_COUNT) {
      return [];
    }
    const stickers: MascotSticker[] = [];
    for (const entry of entries) {
      if (entry === null || typeof entry !== "object") {
        return [];
      }
      const candidate = entry as { file?: unknown; name?: unknown };
      if (!MascotStickerPolicy.validFile(candidate.file)
        || stickers.some((sticker: MascotSticker): boolean => sticker.file === candidate.file)
        || typeof candidate.name !== "string") {
        return [];
      }
      const name: string = candidate.name.trim();
      if (name.length === 0 || TextPolicy.characters(name).length > MASCOT_STICKER_MAX_NAME_CHARACTERS) {
        return [];
      }
      stickers.push({ file: candidate.file as string, name: name });
    }
    return stickers;
  }

  /** 清单里的文件名只能是 `01.webp` 这种两位数字加扩展名，不会带出目录。 */
  static validFile(file: unknown): boolean {
    return typeof file === "string" && FILE_PATTERN.test(file);
  }

  /**
   * 表情面板的页签顺序：有表情包时「水杉娘」挪到最前，其余页签保持原来的相对顺序；读不到表情包（清单无效或包里没有素材）时去掉它，其余照旧。
   */
  static tabOrder<T>(tabs: T[], panelOf: (tab: T) => string, available: boolean): T[] {
    const others: T[] = tabs.filter((tab: T): boolean => panelOf(tab) !== MASCOT_STICKER_PANEL);
    if (!available) {
      return others;
    }
    const stickers: T[] = tabs.filter((tab: T): boolean => panelOf(tab) === MASCOT_STICKER_PANEL);
    return stickers.concat(others);
  }

  /** 打开表情面板时显示哪一页：有表情包时是「水杉娘」，否则还是原来那一页。 */
  static openingPanel(current: string, available: boolean): string {
    return available ? MASCOT_STICKER_PANEL : current;
  }

  /** 网格列数：至少五列，更宽时按每格约 72vp 增加；宽度还没量出来时按五列。 */
  static columns(widthVp: number): number {
    if (!Number.isFinite(widthVp) || widthVp <= 0) {
      return MASCOT_STICKER_MIN_COLUMNS;
    }
    return Math.max(MASCOT_STICKER_MIN_COLUMNS, Math.floor(widthVp / MASCOT_STICKER_CELL_VP));
  }

  /** 正方形格子的边长：宽度扣掉列间距后平分给每一列。 */
  static cellSideVp(widthVp: number, columns: number, gapVp: number): number {
    if (!Number.isFinite(widthVp) || widthVp <= 0 || columns < 1) {
      return 0;
    }
    const gap: number = Number.isFinite(gapVp) ? Math.max(0, gapVp) : 0;
    return Math.max(0, Math.floor((widthVp - gap * (columns - 1)) / columns));
  }

  /** 图片的解码边长：格子扣掉四周留白，至少 1vp，让 ArkUI 按显示大小下采样而不是解出整张 320px。 */
  static decodeSideVp(cellSideVp: number): number {
    return Math.max(1, cellSideVp - MASCOT_STICKER_INSET_VP * 2);
  }

  /** 每格的读屏文字，例如「水杉娘 开心」。 */
  static accessibilityText(sticker: MascotSticker): string {
    return `${MASCOT_STICKER_PACK_NAME} ${sticker.name}`;
  }

  /** 底栏页签的读屏文字。 */
  static tabAccessibilityText(selected: boolean): string {
    return selected ? "水杉娘表情包，已选中" : "水杉娘表情包";
  }

  /** 复制到剪贴板之后在候选栏显示的提示，与 iOS 同一句。 */
  static copiedNotice(sticker: MascotSticker): string {
    return `已复制${MASCOT_STICKER_PACK_NAME}「${sticker.name}」，长按输入框粘贴发送`;
  }

  /** 剪贴板服务给出的错误码归到哪一种失败；不认识的码一律算写入失败。 */
  static clipboardFailure(code: number | undefined): MascotStickerFailure {
    if (code === PASTEBOARD_BUSY_CODE) {
      return MascotStickerFailure.CLIPBOARD_BUSY;
    }
    if (code === PASTEBOARD_PROHIBITED_CODE) {
      return MascotStickerFailure.CLIPBOARD_PROHIBITED;
    }
    return MascotStickerFailure.CLIPBOARD_FAILED;
  }

  /** 复制失败时候选栏的提示，说清楚是哪一步没成。 */
  static failureNotice(failure: MascotStickerFailure): string {
    switch (failure) {
      case MascotStickerFailure.IMAGE_UNREADABLE:
        return "表情包复制失败：图片读不出来";
      case MascotStickerFailure.CLIPBOARD_BUSY:
        return "表情包复制失败：剪贴板正忙，请重试";
      case MascotStickerFailure.CLIPBOARD_PROHIBITED:
        return "表情包复制失败：系统不允许写入剪贴板";
      default:
        return "表情包复制失败，请重试";
    }
  }
}
