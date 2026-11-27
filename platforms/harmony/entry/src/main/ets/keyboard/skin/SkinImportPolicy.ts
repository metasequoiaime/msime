/**
 * Where a picked skin folder is allowed to land.
 *
 * The source lets a user drop a skin into the client's skin folder and offers a button that opens
 * it. That button exists on this host too and was permanently disabled, because the folder lives in
 * the application sandbox where no file manager can reach it — the catalog scans it and reads from
 * it, and nobody could put anything in.
 *
 * A picker replaces the folder: the user chooses a skin folder wherever it actually is, and it is
 * copied in. The goal is the source's; only the mechanism is this platform's, because this platform
 * has no user-visible folder to open.
 *
 * The name comes from a folder the user chose, which means it comes from outside. A skin called
 * `../../engine` would write over the staged dictionary, and one called `.` or `..` would mean the
 * skins root itself, so the name is checked here rather than trusted because it looks like a folder
 * name.
 */

/** Long enough for any real skin, short enough that the path stays within the filesystem's limit. */
const MAX_NAME_LENGTH: number = 64;

export interface SkinImportFeedback {
  publish: boolean;
  message: string;
}

export class SkinImportPolicy {
  /** 取消不通知；成功后提示手动强制刷新，避免同名图片保留缓存。 */
  static feedback(
    ok: boolean, value: string | undefined, error: string | undefined, sharingAvailable: boolean
  ): SkinImportFeedback {
    if (!ok) return { publish: false, message: error || "导入皮肤失败" };
    if (!value) return { publish: false, message: "" };
    if (!sharingAvailable) {
      return { publish: false, message: "皮肤已导入设置页，请点击刷新皮肤；当前无法同步到输入法" };
    }
    return { publish: true, message: "皮肤已导入，请点击刷新皮肤后选择" };
  }

  /** 原生库区分皮肤校验失败和存储失败，避免把存储问题说成格式错误。 */
  static importError(code: string): string {
    if (code === "skin_name" || code === "skin_manifest") {
      return "皮肤格式有误：请选择含 skin.toml 的文件夹，文件夹名须与文件内 id 一致";
    }
    return "导入皮肤失败";
  }
  /**
   * The folder name to import under, or null when the pick cannot be used.
   *
   * A separator of either kind is refused outright rather than stripped: a name that has to be
   * rewritten before it is safe is not a name the user recognises, and silently importing
   * `a/b` as `ab` would leave them looking for something that is not there.
   */
  static destinationName(picked: string | null | undefined): string | null {
    if (picked === null || picked === undefined) {
      return null;
    }
    const name: string = picked.trim();
    if (name.length === 0 || name.length > MAX_NAME_LENGTH) {
      return null;
    }
    if (name === "." || name === "..") {
      return null;
    }
    if (name.includes("/") || name.includes("\\")) {
      return null;
    }
    for (const character of name) {
      const code: number = character.codePointAt(0) ?? 0;
      // Control characters reach a log, a settings page and a filename; none of them want one.
      if (code < 0x20 || code === 0x7f) {
        return null;
      }
    }
    return name;
  }

  /**
   * The last path segment of a picked URI, which is the folder the user chose.
   *
   * Written against the segment rather than the whole URI because the rest of it — the scheme, the
   * provider, the encoded path — is the picker's business and differs between providers.
   */
  static pickedName(uri: string | null | undefined): string | null {
    if (uri === null || uri === undefined) {
      return null;
    }
    const withoutQuery: string = uri.split("?")[0];
    const trimmed: string = withoutQuery.endsWith("/")
      ? withoutQuery.substring(0, withoutQuery.length - 1)
      : withoutQuery;
    const segments: string[] = trimmed.split("/");
    const last: string | undefined = segments[segments.length - 1];
    if (last === undefined) {
      return null;
    }
    let decoded: string = last;
    try {
      decoded = decodeURIComponent(last);
    } catch (error) {
      // A malformed escape is not worth failing the import over; the raw segment still has to pass
      // destinationName, which is where anything dangerous is refused.
      decoded = last;
    }
    return SkinImportPolicy.destinationName(decoded);
  }
}
