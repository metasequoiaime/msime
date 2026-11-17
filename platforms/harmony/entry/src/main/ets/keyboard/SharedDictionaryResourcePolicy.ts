/** 日文、五笔的共享资源只由设置应用安装；迁移完成前保留旧目录。 */
export const SHARED_DICTIONARY_PACKS: string[] = ["japanese", "wubi"];

export interface DictionaryResourceAccess {
  installed(root: string, pack: string): boolean;
  present(root: string, pack: string): boolean;
  adopt(root: string, pack: string, source: string): Promise<boolean>;
  remove(root: string, pack: string): void;
}

export class SharedDictionaryResourcePolicy {
  static root(groupRoot: string): string {
    return `${groupRoot}/dictionary-resources`;
  }

  static async migrate(
    root: string,
    legacyRoots: string[],
    access: DictionaryResourceAccess,
  ): Promise<void> {
    for (const pack of SHARED_DICTIONARY_PACKS) {
      if (!access.installed(root, pack)) {
        for (const legacy of legacyRoots) {
          if (legacy === root || !access.present(legacy, pack)) continue;
          if (await access.adopt(root, pack, `${legacy}/resource-packs/${pack}`)) break;
        }
      }
      // 收编失败时不删除任何旧文件；校验、原子发布和同文件系统改名由 Rust 负责。
      if (!access.installed(root, pack)) continue;
      for (const legacy of legacyRoots) {
        if (legacy !== root && access.present(legacy, pack)) access.remove(legacy, pack);
      }
    }
  }

  static removePrivateCopies(
    root: string,
    privateRoot: string,
    access: DictionaryResourceAccess,
  ): void {
    if (root === privateRoot) return;
    for (const pack of SHARED_DICTIONARY_PACKS) {
      if (access.installed(root, pack) && access.present(privateRoot, pack)) {
        access.remove(privateRoot, pack);
      }
    }
  }
}
