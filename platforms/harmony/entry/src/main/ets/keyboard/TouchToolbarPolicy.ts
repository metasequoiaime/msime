/**
 * 手机键盘空闲工具栏的工具开关和显示方式（共享偏好 `harmony_toolbar`），以及由它们决定的工具条显不显示。
 *
 * 规则与 Android 的 `MSIMEInputService.toolbarRowHidden` 相同：显示方式选了「隐藏」，或者工具全关（这一行只剩两端的标志键和收起键），空闲时整行不显示、键盘变矮；「随」入口（`randomDigits`）只在中文方案的键盘上算一个工具，与 Android 的 `randomDigitsKeepsToolbarRow` 相同；组字时候选栏照常显示；有面板盖住按键时临时显示，因为标志键是回到按键的那个出口。放在视图旁边的纯 TypeScript 里，扩展 ability 在视图存在之前定面板高度也用同一条规则，`tests/run.sh` 能检查它。
 */

/** 文档里 `harmony_toolbar` 的样子：每一项都可能缺，缺的按默认。 */
export interface ToolbarToolsDocument {
  emoji?: boolean;
  phrase?: boolean;
  clipboard?: boolean;
  skin?: boolean;
  scheme?: boolean;
  hidden?: boolean;
  random_digits?: boolean;
}

/** 工具栏上夹在标志键和收起键之间的五个工具和「随」入口各显不显示，以及显示方式是不是「隐藏」。 */
export interface ToolbarTools {
  readonly emoji: boolean;
  readonly phrase: boolean;
  readonly clipboard: boolean;
  readonly skin: boolean;
  readonly scheme: boolean;
  readonly hidden: boolean;
  /** 中文键盘工具栏上的「随」入口（#10254），默认不显示；点它打乱数字九宫格是视图运行时的开关，不在这里。 */
  readonly randomDigits: boolean;
}

export class TouchToolbarPolicy {
  /** 与 client-core 的 `HarmonyToolbarPreferences::default` 相同：五个工具全在、不隐藏、没有「随」，就是有这组设置之前的鸿蒙键盘。 */
  static readonly DEFAULT: ToolbarTools = {
    emoji: true,
    phrase: true,
    clipboard: true,
    skin: true,
    scheme: true,
    hidden: false,
    randomDigits: false,
  };

  /** 把文档里的值读成完整的开关，缺的或不是布尔值的项取默认值。 */
  static fromDocument(document: ToolbarToolsDocument | null | undefined): ToolbarTools {
    const fallback: ToolbarTools = TouchToolbarPolicy.DEFAULT;
    if (document === null || document === undefined) {
      return fallback;
    }
    return {
      emoji: TouchToolbarPolicy.flag(document.emoji, fallback.emoji),
      phrase: TouchToolbarPolicy.flag(document.phrase, fallback.phrase),
      clipboard: TouchToolbarPolicy.flag(document.clipboard, fallback.clipboard),
      skin: TouchToolbarPolicy.flag(document.skin, fallback.skin),
      scheme: TouchToolbarPolicy.flag(document.scheme, fallback.scheme),
      hidden: TouchToolbarPolicy.flag(document.hidden, fallback.hidden),
      randomDigits: TouchToolbarPolicy.flag(document.random_digits, fallback.randomDigits),
    };
  }

  /**
   * 空闲时不画工具栏那一行：选了「隐藏」，或者工具全关了。
   *
   * @param chineseScheme 当前方案写中文（`SchemeTraits.isChinese`）：「随」入口只在中文方案的键盘上画，所以只有这时它让这一行留着；看方案而不看此刻的中/英和临时模式，切换时面板高度不跳，与 Android 相同
   */
  static rowHidden(tools: ToolbarTools, chineseScheme: boolean): boolean {
    return (
      tools.hidden ||
      !(
        tools.emoji ||
        tools.phrase ||
        tools.clipboard ||
        tools.skin ||
        tools.scheme ||
        (tools.randomDigits && chineseScheme)
      )
    );
  }

  /**
   * 工具条（工具栏、候选栏或高度条那一行）这一刻显不显示。
   *
   * @param rowHidden `rowHidden` 的结果
   * @param idle 没有组字、候选、提示这些要占这一行的内容
   * @param panelOpen 有面板盖住了按键：这时要有标志键才能回到按键，所以这一行临时显示
   */
  static stripShown(rowHidden: boolean, idle: boolean, panelOpen: boolean): boolean {
    return !rowHidden || !idle || panelOpen;
  }

  private static flag(value: boolean | undefined, fallback: boolean): boolean {
    return typeof value === "boolean" ? value : fallback;
  }
}
