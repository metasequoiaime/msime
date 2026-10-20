import fs from '@ohos.file.fs';
import fileUri from '@ohos.file.fileuri';
import pasteboard from '@ohos.pasteboard';
import util from '@ohos.util';
import { BusinessError } from '@ohos.base';
import { image } from '@kit.ImageKit';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { NativeCleanup } from '../NativeCleanup';
import {
  MASCOT_STICKER_DIRECTORY, MASCOT_STICKER_MANIFEST, MASCOT_STICKER_MAX_IMAGE_BYTES,
  MASCOT_STICKER_MAX_MANIFEST_BYTES, MascotSticker, MascotStickerFailure, MascotStickerPolicy
} from './MascotStickerPolicy';

/**
 * 贴纸网格的懒加载数据源。表情包随包、装好后不再变，所以不发变化通知。用 `LazyForEach` 而不是 `ForEach`，是为了只给屏幕上（加缓存行）的格子建 `Image`，划出去的格子连同解码出的位图一起释放，输入法扩展的内存里不会同时留着 66 张图。
 */
export class MascotStickerSource implements IDataSource {
  private readonly stickers: MascotSticker[];

  constructor(stickers: MascotSticker[]) {
    this.stickers = stickers;
  }

  totalCount(): number {
    return this.stickers.length;
  }

  getData(index: number): MascotSticker {
    return this.stickers[index];
  }

  registerDataChangeListener(listener: DataChangeListener): void {
    // 数据不变，没有要通知的。
  }

  unregisterDataChangeListener(listener: DataChangeListener): void {
    // 数据不变，没有要通知的。
  }
}

/**
 * 随包的水杉娘表情包：读清单、给出每张图的地址，以及把一张复制到系统剪贴板。
 *
 * `@ohos.inputMethodEngine` 的 `InputClient` 只能插入文字（`insertText` 等），没有把图片或富内容交给输入框的接口，所以点一张是把它解码成 PixelMap 写进系统剪贴板，由用户在输入框里长按粘贴。写剪贴板不需要权限，读才需要 `ohos.permission.READ_PASTEBOARD`。
 */
export class MascotStickerStore {
  private directory: string = '';
  /** 读过一次的清单；读不到或无效时是空表，同样缓存，不在每次打开面板时重读。 */
  private loaded: MascotSticker[] | null = null;
  private readonly uris: Map<string, string> = new Map<string, string>();

  /** 扩展的 `resourceDir`：`stage-resources.sh` 把表情包放进 resfile，安装时解压到这里。 */
  use(resourceDir: string): void {
    this.directory = `${resourceDir}/${MASCOT_STICKER_DIRECTORY}`;
    this.loaded = null;
    this.uris.clear();
  }

  /** 清单里的全部贴纸，按清单顺序。清单无效、或有一张图缺失或超过上限时整份不用，返回空表。 */
  list(): MascotSticker[] {
    if (this.loaded !== null) {
      return this.loaded;
    }
    if (this.directory.length === 0) {
      return [];
    }
    let stickers: MascotSticker[] = MascotStickerPolicy.parse(MascotStickerStore.readText(
      `${this.directory}/${MASCOT_STICKER_MANIFEST}`, MASCOT_STICKER_MAX_MANIFEST_BYTES));
    const missing: MascotSticker | undefined =
      stickers.find((sticker: MascotSticker): boolean => !this.present(sticker));
    if (missing !== undefined) {
      KeyboardLog.error(`mascot sticker ${missing.file} missing or oversized; pack not offered`);
      stickers = [];
    }
    this.loaded = stickers;
    return stickers;
  }

  /** 交给 ArkUI `Image` 的沙箱文件 URI；解码和缓存都由 ArkUI 负责。 */
  uri(sticker: MascotSticker): string {
    const cached: string | undefined = this.uris.get(sticker.file);
    if (cached !== undefined) {
      return cached;
    }
    const uri: string = fileUri.getUriFromPath(this.path(sticker));
    this.uris.set(sticker.file, uri);
    return uri;
  }

  /** 把一张贴纸解码成 PixelMap（保留透明）写进系统剪贴板，返回要在候选栏显示的提示：成功时是怎么粘贴，失败时是哪一步没成。 */
  async copy(sticker: MascotSticker): Promise<string> {
    const pixelMap: image.PixelMap | null = await this.decode(sticker);
    if (pixelMap === null) {
      return MascotStickerPolicy.failureNotice(MascotStickerFailure.IMAGE_UNREADABLE);
    }
    try {
      const data: pasteboard.PasteData = pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pixelMap);
      await pasteboard.getSystemPasteboard().setData(data);
      return MascotStickerPolicy.copiedNotice(sticker);
    } catch (error) {
      KeyboardLog.error(`mascot sticker copy failed: ${describeError(error)}`);
      return MascotStickerPolicy.failureNotice(
        MascotStickerPolicy.clipboardFailure((error as BusinessError).code));
    } finally {
      // setData 完成时剪贴板服务已经收下了位图，本进程这份可以释放。
      pixelMap.release().catch((error: Object): void => {
        KeyboardLog.error(`mascot sticker pixel map release failed: ${describeError(error)}`);
      });
    }
  }

  private path(sticker: MascotSticker): string {
    return `${this.directory}/${sticker.file}`;
  }

  private present(sticker: MascotSticker): boolean {
    try {
      const stat: fs.Stat = fs.statSync(this.path(sticker));
      return stat.isFile() && stat.size > 0 && stat.size <= MASCOT_STICKER_MAX_IMAGE_BYTES;
    } catch {
      return false;
    }
  }

  /** 按原尺寸解码一张，交给剪贴板的要是完整的图；先按打开的描述符查大小，超过上限就不解码。 */
  private async decode(sticker: MascotSticker): Promise<image.PixelMap | null> {
    let handle: fs.File | null = null;
    let source: image.ImageSource | null = null;
    try {
      handle = fs.openSync(this.path(sticker), fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > MASCOT_STICKER_MAX_IMAGE_BYTES) {
        return null;
      }
      source = image.createImageSource(handle.fd);
      const options: image.DecodingOptions = { desiredPixelFormat: image.PixelMapFormat.RGBA_8888 };
      return await source.createPixelMap(options);
    } catch (error) {
      KeyboardLog.error(`mascot sticker ${sticker.file} unreadable: ${describeError(error)}`);
      return null;
    } finally {
      if (source !== null) {
        await source.release().catch((error: Object): void => {
          KeyboardLog.error(`mascot sticker image source release failed: ${describeError(error)}`);
        });
      }
      if (handle !== null) {
        NativeCleanup.close(handle);
      }
    }
  }

  /** 读随包的小文本：先按打开的描述符确认是普通文件且不超过上限，再整份读出。 */
  private static readText(path: string, maximumBytes: number): string | null {
    let handle: fs.File | null = null;
    try {
      handle = fs.openSync(path, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > maximumBytes) {
        return null;
      }
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) {
        return null;
      }
      return util.TextDecoder.create('utf-8').decodeToString(new Uint8Array(buffer, 0, count));
    } catch (error) {
      // 没跑 stage-resources.sh 打出的包里没有表情包，面板照常工作，只是不显示这个页签。
      KeyboardLog.error(`mascot sticker manifest unavailable: ${describeError(error)}`);
      return null;
    } finally {
      if (handle !== null) {
        NativeCleanup.close(handle);
      }
    }
  }
}
