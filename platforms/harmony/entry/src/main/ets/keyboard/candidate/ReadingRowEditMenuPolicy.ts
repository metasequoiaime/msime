import { TextPolicy } from "../TextPolicy";

/** 与 iOS 拼写菜单一致，三项动作直接编辑组字，不借用会翻候选的硬件 Home/End。 */
export enum ReadingRowEditAction {
  START,
  END,
  DELETE_FORWARD,
}

export interface ReadingRowEditItem {
  action: ReadingRowEditAction;
  title: string;
  enabled: boolean;
}

export interface ReadingRowEditTarget {
  moveSpellingHome(): void;
  moveSpellingEnd(): void;
  deleteForward(): void;
}

export class ReadingRowEditMenuPolicy {
  static items(editing: string, caret: number): ReadingRowEditItem[] {
    const length: number = TextPolicy.characters(editing).length;
    const position: number = Number.isFinite(caret) ? Math.max(0, Math.min(length, caret)) : length;
    return [
      { action: ReadingRowEditAction.START, title: "光标移到开头", enabled: position > 0 },
      { action: ReadingRowEditAction.END, title: "光标移到末尾", enabled: position < length },
      {
        action: ReadingRowEditAction.DELETE_FORWARD,
        title: "删除光标后的字母",
        enabled: position < length,
      },
    ];
  }

  static dispatch(action: ReadingRowEditAction, target: ReadingRowEditTarget): void {
    switch (action) {
      case ReadingRowEditAction.START:
        target.moveSpellingHome();
        break;
      case ReadingRowEditAction.END:
        target.moveSpellingEnd();
        break;
      case ReadingRowEditAction.DELETE_FORWARD:
        target.deleteForward();
        break;
    }
  }
}
