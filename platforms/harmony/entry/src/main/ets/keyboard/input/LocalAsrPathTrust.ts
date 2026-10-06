/**
 * 本地语音模型路径的链接判定，不依赖 `@ohos.file.fs`，因此能在 node 下测试；`workers/LocalAsrWorker.ets` 负责把 `lstatSync`/`statSync` 的结果交给它。
 *
 * 规则与 `crates/path-trust/src/lib.rs` 一致。HarmonyOS 的 Rust 目标是 `target_os = "linux"`，所以这里用的是 Linux 那一条：路径上的符号链接一律拒绝，只有最后一级以上至多一个「仅 root 可改的链接」例外——链接本身属于 root，所在目录也属于 root 且组和其他用户都不可写。最后一级永远不能是链接。
 */

/** 一次 `lstat` 或 `stat` 的结果；路径不存在或读不到时由调用方返回 null。 */
export interface PathTrustStat {
  isSymbolicLink: boolean;
  isDirectory: boolean;
  isFile: boolean;
  uid: number;
  mode: number;
}

/** 按路径取状态。`lstat` 不跟随最后一级链接，`stat` 跟随。 */
export type PathTrustLookup = (path: string) => PathTrustStat | null;

const GROUP_OR_OTHER_WRITABLE: number = 0o022;

export class LocalAsrPathTrust {
  /**
   * `path` 必须是绝对路径，每一级都存在，中间各级是目录，最后一级是普通文件（`leafIsFile`）或目录。
   *
   * 对应 `path_trust::reject_symlinked_components`，只是这里每一级都必须已经存在：模型目录是读取对象，不是待创建的位置。
   */
  static trusted(path: string, leafIsFile: boolean, lstat: PathTrustLookup, stat: PathTrustLookup): boolean {
    if (!path.startsWith("/")) return false;
    const parts: string[] = path.split("/").filter((part: string): boolean => part.length > 0);
    if (parts.length === 0) return !leafIsFile && LocalAsrPathTrust.directory(stat("/"));
    let current: string = "";
    let sawSystemLink: boolean = false;
    for (let index = 0; index < parts.length; index++) {
      const parent: string = current.length === 0 ? "/" : current;
      current += `/${parts[index]}`;
      const last: boolean = index + 1 === parts.length;
      const found: PathTrustStat | null = lstat(current);
      if (found === null) return false;
      if (found.isSymbolicLink) {
        if (last || sawSystemLink || !LocalAsrPathTrust.rootOnlyLink(found, stat(parent))) return false;
        sawSystemLink = true;
        // 跟随这个受信任的链接后，它指向的仍须是目录。
        if (!LocalAsrPathTrust.directory(stat(current))) return false;
        continue;
      }
      if (last ? (leafIsFile ? !found.isFile : !found.isDirectory) : !found.isDirectory) return false;
    }
    return true;
  }

  /** 对应 `path_trust::is_root_only_link`：除了 root 谁都不能创建、替换或删除这样的链接。粘滞位的全局可写目录（如 `/tmp`）里的链接不算。 */
  static rootOnlyLink(link: PathTrustStat, parent: PathTrustStat | null): boolean {
    return link.isSymbolicLink && link.uid === 0 && parent !== null && parent.isDirectory &&
      parent.uid === 0 && (parent.mode & GROUP_OR_OTHER_WRITABLE) === 0;
  }

  private static directory(found: PathTrustStat | null): boolean {
    return found !== null && found.isDirectory;
  }
}
