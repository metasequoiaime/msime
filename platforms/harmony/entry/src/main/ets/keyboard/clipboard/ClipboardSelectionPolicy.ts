/**
 * 本机剪贴板历史的多选规则，与 Android 的 `ClipboardSelectionPolicy` 逐条相同（#10411）：最多选 20 项，选中 1 到 4 项时可以按 ①②③④ 的顺序一次插入，中间不加分隔符。鸿蒙的多选目前只有插入，上限仍按 20 项，以后补删除时不用再改规则。
 */
export class ClipboardSelectionPolicy {
  static readonly MAX_SELECTIONS: number = 20;
  static readonly MAX_INSERTIONS: number = 4;
  /** 选中项超过插入上限、「插入」置灰时给的提示。 */
  static readonly INSERTION_LIMIT_HINT: string = `一次最多插入 ${ClipboardSelectionPolicy.MAX_INSERTIONS} 项，取消几项后再插入`;
  /** 带圈数字 ① 的码位；⑳ 正好是第 20 个，和选择上限对得上。 */
  private static readonly FIRST_CIRCLED_DIGIT: number = 0x2460;

  static canSelect(selectedCount: number): boolean {
    return selectedCount >= 0 && selectedCount < ClipboardSelectionPolicy.MAX_SELECTIONS;
  }

  static canInsert(selectedCount: number): boolean {
    return selectedCount >= 1 && selectedCount <= ClipboardSelectionPolicy.MAX_INSERTIONS;
  }

  /** 第 `index` 个选中项（从 0 起）的角标：①、②……⑳；超出范围时为空串。 */
  static ordinalLabel(index: number): string {
    if (!Number.isInteger(index) || index < 0 || index >= ClipboardSelectionPolicy.MAX_SELECTIONS) {
      return "";
    }
    return String.fromCharCode(ClipboardSelectionPolicy.FIRST_CIRCLED_DIGIT + index);
  }

  /** `text` 在按点选先后排列的选中项里的序号（从 0 起），没选中时为 -1。取消一项后，排在它后面的各项序号自然依次前移。 */
  static ordinal(selected: string[], text: string): number {
    return selected.indexOf(text);
  }

  /** 点按一条后的选中项：已选中的取消，未选中且没到上限的追加到末尾，到了上限的原样返回（同一个数组）。 */
  static toggled(selected: string[], text: string): string[] {
    const index: number = selected.indexOf(text);
    if (index >= 0) {
      return selected.filter((_entry: string, position: number): boolean => position !== index);
    }
    return ClipboardSelectionPolicy.canSelect(selected.length) ? selected.concat([text]) : selected;
  }

  /** 按选中顺序把各条首尾相接成一次插入的文字，中间不加分隔符。 */
  static insertionText(texts: string[]): string {
    return texts.join("");
  }

  /** 多选时的提示行：超过插入上限时说明「插入」为什么点不了，否则说明怎么用。 */
  static notice(selectedCount: number): string {
    return selectedCount > ClipboardSelectionPolicy.MAX_INSERTIONS
      ? ClipboardSelectionPolicy.INSERTION_LIMIT_HINT
      : "点按选择，按 ①②③④ 的顺序插入";
  }

  /** 选中卡片的读屏前缀；没选中（`index` 为 -1）时为空。 */
  static selectedDescription(index: number): string {
    return index < 0 ? "" : `已选择第 ${index + 1} 项；`;
  }
}
