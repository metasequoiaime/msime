import type { EditionInfo } from "../core/host-capabilities";

/** 设置页对用户说的 Linux 输入法名字和命令，随本安装包所属的版本（与 `platforms/linux/scripts/edition_linux.py` 改写脚本时用的同一组名字）。 */
export interface LinuxEditionNames {
  /** 产品名，例如「水杉输入法」「水杉五笔」；版本没报告名字时是中性的「本输入法」。 */
  name: string;
  /** Fcitx5 输入法列表里的写法，带引号；full 附上英文界面里的名字。 */
  fcitx5Label: string;
  /** IBus 输入源列表里的写法（`IBUS_LONGNAME`，「Metasequoia <产品名>」），带引号。 */
  ibusLabel: string;
  /** 首次配置命令：full 是 `msime-linux-setup`，其他版本是 `msime-linux-<id>-setup`。 */
  setupCommand: string;
}

/**
 * 本版本在 Linux 上的名字。设置前端在各版本之间共用，写死 full 的名字会让五笔、拼音等版本的用户去找一个不存在的条目和命令。宿主只在不是 full 时报告版本（`HostCapabilities.edition`），没有版本信息就是 full。包名的规则（不是 full 的版本是 `msime-linux-<id>`）由 `shared/contracts/editions.schema.json` 规定并由 `scripts/test-editions.py` 检查。
 */
export function linuxEditionNames(edition?: EditionInfo): LinuxEditionNames {
  if (!edition || edition.id === "full")
    return {
      name: "水杉输入法",
      fcitx5Label: "「水杉输入法」（英文界面显示为「MSIME」）",
      ibusLabel: "「Metasequoia 水杉输入法」",
      setupCommand: "msime-linux-setup",
    };
  const setupCommand = `msime-linux-${edition.id}-setup`;
  const name = edition.display_name;
  if (!name)
    return {
      name: "本输入法",
      fcitx5Label: "本版本的输入法条目",
      ibusLabel: "本版本的输入源（名字以 Metasequoia 开头）",
      setupCommand,
    };
  return {
    name,
    fcitx5Label: `「${name}」`,
    ibusLabel: `「Metasequoia ${name}」`,
    setupCommand,
  };
}
