/** 一个路径此刻的文件身份。读不到（不存在、没有权限）时由取值函数返回 null。 */
export interface SkinFileStat {
  /** inode 号，`fs.Stat.ino` 是 bigint，这里存它的十进制文本。 */
  readonly ino: string;
  /** 修改时间：有纳秒（`mtimeNs`）时用纳秒，否则用秒。 */
  readonly mtime: string;
  readonly size: number;
}

/** 对一个路径做一次 stat。键盘上用 `fs.statSync`，测试里用 node 的 `fs.statSync`。 */
export type SkinFileStatter = (path: string) => SkinFileStat | null;

/**
 * 发现选中的候选皮肤包在原处被同名替换。
 *
 * 设置应用里重装图库作品、皮肤编辑器覆盖保存或导入同名包时，选择不变，偏好文档的修订号也不变，键盘按修订号重载就看不到它。这里只对选中的包目录、它的 `skin.toml` 和实际画出的装饰图、背景图各做一次 stat，比较 inode、修改时间和大小：共享状态镜像导入 `skins` 时整棵目录先复制到旁边再改名换入（`SharedStateMirror.copy`），设置应用一侧的安装也是暂存加改名（client-services 的 `candidate_community::install`），包目录的 inode 一定变；原地改写文件时修改时间或大小变。不列目录、不读文件内容。
 */
export class CandidateSkinPackageIdentity {
  /** 浅色、深色槽位选中的包目录和它的 `skin.toml`，两个槽位是同一款时只取一次。不像包名的值（空、含 `/`、`.` 或 `..`）跳过，不拿去拼路径。 */
  static packagePaths(skinsRoot: string, ids: readonly (string | null | undefined)[]): string[] {
    const paths: string[] = [];
    for (const id of ids) {
      if (!CandidateSkinPackageIdentity.packageName(id)) continue;
      const folder: string = `${skinsRoot}/${id}`;
      if (paths.includes(folder)) continue;
      paths.push(folder, `${folder}/skin.toml`);
    }
    return paths;
  }

  /** 画出来的包里那几张图（装饰图、背景图），没有的跳过，重复的只取一次。目录 inode 不变、只原地改写了图片时靠它们发现。 */
  static imagePaths(
    skinsRoot: string,
    id: string | null,
    relatives: readonly (string | null)[],
  ): string[] {
    if (!CandidateSkinPackageIdentity.packageName(id)) return [];
    const paths: string[] = [];
    for (const relative of relatives) {
      if (relative === null || relative.length === 0) continue;
      const path: string = `${skinsRoot}/${id}/${relative}`;
      if (!paths.includes(path)) paths.push(path);
    }
    return paths;
  }

  /** 每个路径一次 stat，拼成 `inode:修改时间:大小;`，读不到的记作 `-;`。两次结果相同就说明这些文件都没换过。 */
  static identity(paths: readonly string[], stat: SkinFileStatter): string {
    let identity: string = "";
    for (const path of paths) {
      const info: SkinFileStat | null = stat(path);
      identity += info === null ? "-;" : `${info.ino}:${info.mtime}:${info.size};`;
    }
    return identity;
  }

  private static packageName(id: string | null | undefined): id is string {
    return (
      typeof id === "string" && id.length > 0 && !id.includes("/") && id !== "." && id !== ".."
    );
  }
}
