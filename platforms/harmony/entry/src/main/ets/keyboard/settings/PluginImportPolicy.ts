/**
 * 「扩展」页在 `msime_client_plugins` 检查之前，把选中的包复制多少到它的暂存目录。
 *
 * 选择器交回的是原生库打不开的文档 URI，所以选中的内容要先复制到应用的缓存里。桌面外壳没有这道复制：client-services 的 `plugins::import` 自己读取选中的文件夹或压缩包，复制时到上限就停。暂存复制如果选中什么就复制什么，就会绕开这些上限，而且像「下载」这样选错的目录会被整个复制，可能把设备塞满，之后 client-services 才拒绝它。所以这里的复制也停在同样的上限，取自 `crates/client-services/src/plugins/import.rs` 和 `plugins.rs`；包必须满足的每一条规则，仍由 client-services 在暂存副本上检查。
 */

/** `plugins::MAX_PACK_FILES`. */
const MAX_PACK_FILES: number = 16;
/** import.rs `MAX_FILE_BYTES`: `music_pack::MAX_TRACK_BYTES`, the largest file any kind allows. */
const MAX_FILE_BYTES: number = 16 * 1024 * 1024;
/** import.rs `MAX_TOTAL_BYTES`: `music_pack::MAX_PACK_BYTES` plus 2 MiB. */
const MAX_TOTAL_BYTES: number = 66 * 1024 * 1024;
/** import.rs `MAX_ARCHIVE_BYTES`. */
const MAX_ARCHIVE_BYTES: number = 80 * 1024 * 1024;

/** 一个拒绝，采用 `msime_client_plugins` 的回答形状，带 client-services 对同一条规则给出的错误码和详情。 */
export interface PluginImportRefusal {
  ok: boolean;
  error: string;
  detail: string;
}

/** What `lstat` says one entry of a picked folder is. */
export enum PickedEntryKind {
  FILE,
  DIRECTORY,
  LINK,
  /** A device, socket or pipe. */
  OTHER,
}

export class PluginImportPolicy {
  /** 选中的压缩包的暂存副本。client-services 按 `.zip` 扩展名判断，并从 `plugin.toml` 读出包 id，所以用不着选中文件自己的名字，那个名字可能很长或带空格。 */
  static readonly ARCHIVE_NAME: string = "pack.zip";
  /** The staged copy of a picked folder, named for the same reason. */
  static readonly FOLDER_NAME: string = "pack";

  /** client-services 复制文件夹时略过的名字（`.DS_Store`、`.git`），所以这里既不检查也不复制。 */
  static hidden(name: string): boolean {
    return name.startsWith(".");
  }

  /** The refusal for a picked archive of `bytes`, or null when it may be copied. */
  static archive(bytes: number): PluginImportRefusal | null {
    return bytes > MAX_ARCHIVE_BYTES
      ? { ok: false, error: "plugin_archive", detail: "压缩包太大" }
      : null;
  }
}

/** The top-level entries of a picked folder, taken one at a time so the scan stops at the first that rules the folder out. */
export class PluginFolderScan {
  /** The regular files to copy, hidden names aside. */
  readonly files: string[] = [];
  private bytes: number = 0;

  /**
   * 接收一个大小为 `bytes` 的非隐藏条目。一旦这个文件夹不可能是包，就返回拒绝，详情与 client-services 复制文件夹时对同一条规则给出的相同；否则返回 null，继续扫描。
   */
  take(name: string, kind: PickedEntryKind, bytes: number): PluginImportRefusal | null {
    if (kind === PickedEntryKind.LINK) {
      return { ok: false, error: "plugin_invalid", detail: `${name} 是符号链接` };
    }
    if (kind === PickedEntryKind.DIRECTORY) {
      return { ok: false, error: "plugin_invalid", detail: `${name} 是子文件夹` };
    }
    if (kind !== PickedEntryKind.FILE) {
      return { ok: false, error: "plugin_invalid", detail: `${name} 不是普通文件` };
    }
    this.files.push(name);
    this.bytes += bytes;
    if (
      this.files.length > MAX_PACK_FILES ||
      bytes > MAX_FILE_BYTES ||
      this.bytes > MAX_TOTAL_BYTES
    ) {
      return { ok: false, error: "plugin_invalid", detail: "扩展包太大" };
    }
    return null;
  }
}
