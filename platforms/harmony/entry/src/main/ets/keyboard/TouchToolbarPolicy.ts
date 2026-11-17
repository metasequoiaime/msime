/**
 * 手机键盘空闲工具栏的工具开关和显示方式（共享偏好 `harmony_toolbar`），以及由它们决定的工具条显不显示。
 *
 * 规则与 Android 的 `MSIMEInputService.toolbarRowHidden` 相同：显示方式选了「隐藏」，或者五个工具全关（这一行只剩两端的标志键和收起键），空闲时整行不显示、键盘变矮；组字时候选栏照常显示；有面板盖住按键时临时显示，因为标志键是回到按键的那个出口。放在视图旁边的纯 TypeScript 里，扩展 ability 在视图存在之前定面板高度也用同一条规则，`tests/run.sh` 能检查它。
 */

/** 文档里 `harmony_toolbar` 的样子：每一项都可能缺，缺的按默认。 */
export interface ToolbarToolsDocument {
  emoji?: boolean;
  phrase?: boolean;
  clipboard?: boolean;
  skin?: boolean;
  scheme?: boolean;
  hidden?: boolean;
}

/** 工具栏上夹在标志键和收起键之间的五个工具各显不显示，以及显示方式是不是「隐藏」。 */
export interface ToolbarTools {
  readonly emoji: boolean;
  readonly phrase: boolean;
  readonly clipboard: boolean;
  readonly skin: boolean;
  readonly scheme: boolean;
  readonly hidden: boolean;
}

/** 空闲工具栏这一刻每一格的宽度和相邻两格的间距，单位 vp。 */
export interface ToolbarCluster {
  readonly cellVp: number;
  readonly gapVp: number;
}

export class TouchToolbarPolicy {
  /** 相邻两个工具之间的间距，与 iOS、Android 的工具栏相同。 */
  static readonly CLUSTER_GAP_VP: number = 8;

  /** 与 client-core 的 `HarmonyToolbarPreferences::default` 相同：五个工具全在、不隐藏，就是有这组设置之前的鸿蒙键盘。 */
  static readonly DEFAULT: ToolbarTools = {
    emoji: true,
    phrase: true,
    clipboard: true,
    skin: true,
    scheme: true,
    hidden: false,
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
    };
  }

  /** 空闲时不画工具栏那一行：选了「隐藏」，或者五个工具全关了。 */
  static rowHidden(tools: ToolbarTools): boolean {
    return (
      tools.hidden ||
      !(tools.emoji || tools.phrase || tools.clipboard || tools.skin || tools.scheme)
    );
  }

  /**
   * 工具条（工具栏或候选栏那一行）这一刻显不显示。
   *
   * @param rowHidden `rowHidden` 的结果
   * @param idle 没有组字、候选、提示这些要占这一行的内容
   * @param panelOpen 有面板盖住了按键：这时要有标志键才能回到按键，所以这一行临时显示
   */
  static stripShown(rowHidden: boolean, idle: boolean, panelOpen: boolean): boolean {
    return !rowHidden || !idle || panelOpen;
  }

  /**
   * 空闲工具栏的排法：标志在行首，其余可见工具和收起键按固定格宽、固定间距聚到行尾，与 iOS、Android 相同，间距不随可见工具的数量变化。
   *
   * 这一行不能滚动。放不下时（窄的浮动键盘，或手机上五个工具加长空格样式的「符」）先把间距均匀收窄到 0，还放不下再把格子均匀收窄，退回原来等分整行的样子，不把收起键挤出去。
   *
   * @param widthVp 这一行除去两侧内边距后的宽度；还没量出来（不大于 0）时按设计尺寸
   * @param cells 这一行画几格，含标志、符和收起
   * @param cellVp 一格的设计宽度（工具按钮的边长，已乘工具条缩放）
   */
  static cluster(widthVp: number, cells: number, cellVp: number): ToolbarCluster {
    const gap: number = TouchToolbarPolicy.CLUSTER_GAP_VP;
    if (!(widthVp > 0) || cells <= 1 || cells * cellVp + (cells - 1) * gap <= widthVp) {
      return { cellVp: cellVp, gapVp: gap };
    }
    if (cells * cellVp <= widthVp) {
      return { cellVp: cellVp, gapVp: (widthVp - cells * cellVp) / (cells - 1) };
    }
    return { cellVp: widthVp / cells, gapVp: 0 };
  }

  private static flag(value: boolean | undefined, fallback: boolean): boolean {
    return typeof value === "boolean" ? value : fallback;
  }
}
