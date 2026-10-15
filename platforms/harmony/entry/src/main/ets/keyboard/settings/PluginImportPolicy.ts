/**
 * How much of a picked pack the 扩展 page copies into its staging directory before `msime_client_plugins` checks it.
 *
 * The picker hands back a document URI the native library cannot open, so the pick is copied into the app's cache first. The desktop shell has no such copy: client-services' `plugins::import` reads the picked folder or archive itself and stops at its bounds while copying. A staging copy that copied whatever was picked would bypass those bounds, and a wrong pick such as Downloads would be copied whole, possibly filling the device, before client-services refused it. So the copy stops at the same bounds, taken from `crates/client-services/src/plugins/import.rs` and `plugins.rs`; every rule a pack must meet is still checked by client-services on the staged copy.
 */

/** `plugins::MAX_PACK_FILES`. */
const MAX_PACK_FILES: number = 16;
/** import.rs `MAX_FILE_BYTES`: `music_pack::MAX_TRACK_BYTES`, the largest file any kind allows. */
const MAX_FILE_BYTES: number = 16 * 1024 * 1024;
/** import.rs `MAX_TOTAL_BYTES`: `music_pack::MAX_PACK_BYTES` plus 2 MiB. */
const MAX_TOTAL_BYTES: number = 66 * 1024 * 1024;
/** import.rs `MAX_ARCHIVE_BYTES`. */
const MAX_ARCHIVE_BYTES: number = 80 * 1024 * 1024;

/** A refusal in the reply shape `msime_client_plugins` uses, with client-services' code and the detail it gives for the same rule. */
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
  /** The staged copy of a picked archive. client-services goes by the `.zip` extension and reads the pack id from plugin.toml, so the picked file's own name, which may be long or carry spaces, is not needed. */
  static readonly ARCHIVE_NAME: string = "pack.zip";
  /** The staged copy of a picked folder, named for the same reason. */
  static readonly FOLDER_NAME: string = "pack";

  /** A name client-services' folder copy leaves behind (`.DS_Store`, `.git`), so it is neither checked nor copied here. */
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
   * Take one entry that is not hidden, of `bytes`. Returns the refusal once the folder cannot be a pack, with the detail client-services' folder copy gives for the same rule, or null to go on.
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
