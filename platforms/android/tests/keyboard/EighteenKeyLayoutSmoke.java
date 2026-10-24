package app.msime.android;

import java.util.List;

/** 十八键全拼作为独立可选入口，沿用全拼方案而非双拼。 */
public final class EighteenKeyLayoutSmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] arguments) {
        KeyboardScheme eighteen = KeyboardScheme.fromPreferenceId("eighteen_key");
        check(eighteen != null, "Android 必须提供十八键全拼入口");
        check(eighteen.touchKeyboardLayout().equals("eighteen_key"), "布局独立保存");
        check(eighteen.engineScheme(AppEdition.FULL).equals("quanpin"), "组词继续归全拼引擎");
        check(eighteen.optIn(), "新增布局由用户启用");
        check(!KeyboardScheme.enabledFromPreferenceIds(null, AppEdition.FULL).contains(eighteen),
            "新安装的默认键盘列表不增加入口");
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("eighteen_key"), AppEdition.FULL)
            .equals(List.of(eighteen)), "已保存的启用列表可以恢复十八键");
        check(eighteen.title().equals("全拼 18 键"), "设置显示十八键全拼");
        check(KeyboardScheme.fromPreferences("quanpin", null, "eighteen_key", AppEdition.FULL)
            == eighteen, "重开后由偏好恢复同一个入口");
        check(TypingSource.resolve(eighteen, false, "none").id().equals("eighteenKey"),
            "十八键统计不能混入二十六键");
        check(TypingSource.resolve(eighteen, true, "none") == TypingSource.ENGLISH,
            "英文回落仍统计为英文");
        List<List<FourteenKeyLayout.Key>> rows = EighteenKeyLayout.rows();
        check(rows.stream().map(List::size).toList().equals(List.of(7, 6, 5)), "字母键分三排");
        String all = rows.stream().flatMap(List::stream).map(FourteenKeyLayout.Key::letters)
            .reduce("", String::concat);
        check(all.length() == 26 && all.chars().distinct().count() == 26,
            "每个英文字母恰好出现一次");
        check(rows.stream().map(row -> row.stream().map(FourteenKeyLayout::face)
                .reduce((left, right) -> left + " " + right).orElse("")).toList()
            .equals(List.of("Q WE RT Y U IO P", "A SD FG H JK L", "Z XC V BN M")),
            "键面按共享十八键全拼分组");
        for (List<FourteenKeyLayout.Key> row : rows) {
            for (FourteenKeyLayout.Key key : row) {
                String id = EighteenKeyLayout.keyId(key);
                check(KeyPressIds.isKnown(id), "每个十八键键位都可写入共享统计");
                check(KeyPressIds.label(id).equals("18 键 " + FourteenKeyLayout.face(key)),
                    "统计标签显示实际十八键键面");
            }
        }
        int layout = KeyboardLayout.EIGHTEEN_KEY_LAYOUT;
        check(KeyboardLayout.resolveTouchLayout(false, false, false, true, 0, "eighteen_key") == layout,
            "引擎十八键网格对应十八键键面");
        check(KeyboardLayout.resolveTouchLayout(false, false, false, false, 0, "eighteen_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT, "偏好本身不能打开键面，本地模式回落确定字母");
        check(KeyboardLayout.drawsKeyGrid(layout), "读音与展开候选走组码分支");
        check(KeyboardLayout.drawsNumberPad(KeyboardLayout.Layer.SYMBOLS, layout, true, false),
            "数字层沿用中文数字页");
        check(!KeyboardLayout.carriesLetterCase(layout), "多义字母键不提供 Shift");
        check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, layout, 760),
            "组码键面不叠加实体数字行");
        check(KeyboardActionRow.layerTitle(layout, true).equals("18键"), "数字层可返回十八键");
        System.out.println("eighteen-key quanpin scheme: passed");
    }
}
