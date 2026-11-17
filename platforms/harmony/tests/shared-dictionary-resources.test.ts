import {
  SETTINGS_ENTRIES,
  KEYBOARD_ENTRIES,
} from "../entry/src/main/ets/keyboard/SharedStateMirrorPolicy";
import {
  DictionaryResourceAccess,
  SHARED_DICTIONARY_PACKS,
  SharedDictionaryResourcePolicy,
} from "../entry/src/main/ets/keyboard/SharedDictionaryResourcePolicy";

function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

class SyntheticAccess implements DictionaryResourceAccess {
  packs = new Set<string>();
  installedPacks = new Set<string>();
  calls: string[] = [];
  fail = false;
  installed(root: string, pack: string): boolean {
    return this.installedPacks.has(`${root}/${pack}`);
  }
  present(root: string, pack: string): boolean {
    return this.packs.has(`${root}/${pack}`);
  }
  async adopt(root: string, pack: string, source: string): Promise<boolean> {
    this.calls.push(`adopt:${source}`);
    if (this.fail) return false;
    this.packs.add(`${root}/${pack}`);
    this.installedPacks.add(`${root}/${pack}`);
    return true;
  }
  remove(root: string, pack: string): void {
    this.calls.push(`remove:${root}/${pack}`);
    this.packs.delete(`${root}/${pack}`);
    this.installedPacks.delete(`${root}/${pack}`);
  }
}

async function run(): Promise<void> {
  const root = SharedDictionaryResourcePolicy.root("/synthetic/group/msime");
  check(root === "/synthetic/group/msime/dictionary-resources", "两端共享同一个资源根目录");
  const legacy = ["/synthetic/private/state", "/synthetic/group/msime/settings/state"];
  for (const pack of SHARED_DICTIONARY_PACKS) {
    check(
      !SETTINGS_ENTRIES.some((item) => item.name === `resource-pack-${pack}`),
      "资源包不再经状态镜像复制",
    );
    check(
      !KEYBOARD_ENTRIES.some((item) => item.name === `resource-pack-${pack}`),
      "键盘不能反向发布资源包",
    );
  }
  const access = new SyntheticAccess();
  for (const pack of SHARED_DICTIONARY_PACKS)
    for (const old of legacy) access.packs.add(`${old}/${pack}`);
  await SharedDictionaryResourcePolicy.migrate(root, legacy, access);
  check(access.packs.size === 2, "迁移后两种资源各只有共享的一份");
  check(access.calls[0].startsWith("adopt:"), "先校验收编再删除旧文件");
  const count = access.calls.length;
  await SharedDictionaryResourcePolicy.migrate(root, legacy, access);
  check(access.calls.length === count, "重复打开不重新收编或复制");
  access.installedPacks.delete(`${root}/japanese`);
  access.packs.delete(`${root}/japanese`);
  await SharedDictionaryResourcePolicy.migrate(root, legacy, access);
  check(!access.present(root, "japanese"), "卸载后不会从旧镜像复活");

  const failed = new SyntheticAccess();
  failed.fail = true;
  failed.packs.add(`${legacy[0]}/japanese`);
  await SharedDictionaryResourcePolicy.migrate(root, legacy, failed);
  check(failed.present(legacy[0], "japanese"), "收编失败保留旧文件");
  check(!failed.calls.some((call) => call.startsWith("remove:")), "失败时不清理源目录");
  // 第一来源无法校验时仍尝试第二来源。
  failed.fail = false;
  failed.packs.add(`${legacy[1]}/japanese`);
  failed.adopt = async (target, pack, source) => {
    failed.calls.push(`adopt:${source}`);
    if (source.startsWith(legacy[0])) return false;
    failed.packs.add(`${target}/${pack}`);
    failed.installedPacks.add(`${target}/${pack}`);
    return true;
  };
  await SharedDictionaryResourcePolicy.migrate(root, legacy, failed);
  check(failed.present(root, "japanese") && failed.packs.size === 1, "可用来源成功后清理两份旧包");

  {
    const keyboard = new SyntheticAccess();
    keyboard.packs.add(`${legacy[0]}/wubi`);
    SharedDictionaryResourcePolicy.removePrivateCopies(root, legacy[0], keyboard);
    check(keyboard.present(legacy[0], "wubi"), "共享包尚未发布时保留私有文件");
    keyboard.installedPacks.add(`${root}/wubi`);
    SharedDictionaryResourcePolicy.removePrivateCopies(root, legacy[0], keyboard);
    check(!keyboard.present(legacy[0], "wubi"), "键盘只清理私有目录");
    check(
      keyboard.calls.every((call) => !call.startsWith(`remove:${root}`)),
      "键盘不写共享目录",
    );
  }
  console.log("鸿蒙共享资源：无复制迁移、失败保留、重复打开、卸载与只读键盘通过");
}
run().catch((error) => {
  throw error;
});
