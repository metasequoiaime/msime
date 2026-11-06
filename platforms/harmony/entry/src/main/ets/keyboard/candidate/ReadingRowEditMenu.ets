import { ReadingRowEditAction, ReadingRowEditItem, ReadingRowEditMenuPolicy } from './ReadingRowEditMenuPolicy';

/** 原生菜单留在键盘窗口内，点外面关闭，禁用项保留位置，读屏沿用菜单项的名称。 */
export class ReadingRowEditMenu {
  static items(editing: string, caret: number, act: (action: ReadingRowEditAction) => void): MenuElement[] {
    return ReadingRowEditMenuPolicy.items(editing, caret).map((item: ReadingRowEditItem): MenuElement => {
      return { value: item.title, enabled: item.enabled, action: (): void => act(item.action) };
    });
  }
}
