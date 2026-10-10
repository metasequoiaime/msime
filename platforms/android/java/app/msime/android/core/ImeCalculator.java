package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.widget.Button;

/**
 * 数字键面的四则运算结果（#5688）：在数字 / 符号键面上打出 `12*12` 这样的算式后，工具栏左侧（品牌键右边）出现「= 144」，点一下把结果上屏。
 *
 * <p>算式从编辑器里光标前的文字读（{@link ArithmeticResultPolicy#MAX_CONTEXT} 个字符），每次选区变化、切换键面时重算，所以无论算式是用九键、26 键的符号层还是左栏的运算符号打出来的都一样。密码框里不读上文，也不显示结果。读到的文字只在内存里算一次，不记录、不保存。
 */
final class ImeCalculator {
    private final MSIMEInputService s;
    private Button chip;
    private ArithmeticResultPolicy.Result result;

    ImeCalculator(MSIMEInputService s) {
        this.s = s;
    }

    /** 工具栏上的结果胶囊；第一次用时建，没有结果时隐藏。 */
    Button chip() {
        if (chip == null) {
            KeyboardPressButton button = new KeyboardPressButton(s);
            button.setKeyboardRole(KeyboardKeyRole.PILL);
            ViewPolicy.setAllCapsFalse(button);
            ViewPolicy.setSingleLineEllipsized(button);
            KeyboardTypographyPolicy.setKeyTextSize(button, 15);
            LayoutPolicy.setHorizontalPaddingDp(button, s, 8);
            ViewPolicy.clearMinimumHeight(button);
            // 工具栏七个键各至少 40 dp，加起来约 294 dp；胶囊再宽就把最右边的「收起」挤出 393 dp 宽的屏幕。长结果在胶囊里截断显示，点按上屏的仍是完整结果，无障碍描述也读完整结果。
            button.setMaxWidth(s.pixels(96));
            ViewPolicy.bindClick(button, () -> {
                s.imeKeyFeedback.playFeedback(button);
                commit();
            });
            s.imeStyler.styleButton(button, KeyboardKeyRole.PILL, s.skin);
            ViewPolicy.hide(button);
            chip = button;
        }
        return chip;
    }

    /** 按光标前的文字重算；不在数字 / 符号键面、没有输入框、密码框或正在组字时清掉结果。 */
    void refresh() {
        ArithmeticResultPolicy.Result next = null;
        if (applies() && s.connection != null && !EditorPolicy.password(s.editorInputType)
                && !s.hasEngineComposition()) {
            CharSequence before = s.connection.getTextBeforeCursor(ArithmeticResultPolicy.MAX_CONTEXT, 0);
            next = ArithmeticResultPolicy.evaluateTrailing(before);
        }
        show(next);
    }

    /** 不重读上文，只在已经离开数字 / 符号键面时收起结果（每次渲染调用，不做 IPC）。 */
    void syncVisibility() {
        if (result != null && !applies()) show(null);
    }

    /** 收起结果（离开输入框时）。 */
    void clear() {
        show(null);
    }

    private boolean applies() {
        return s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
    }

    private void show(ArithmeticResultPolicy.Result next) {
        result = next;
        if (chip == null) return;
        if (next == null) {
            ViewPolicy.hide(chip);
            return;
        }
        chip.setText(next.label());
        chip.setContentDescription("计算结果 " + next.value() + "，点按上屏");
        ViewPolicy.show(chip);
    }

    /** 上屏结果：和九键数字一样走字面上屏（全角模式下转全角），之后按新的上文重算（通常就没有结果了）。 */
    private void commit() {
        ArithmeticResultPolicy.Result current = result;
        if (current == null || s.connection == null) return;
        show(null);
        s.imeLayoutRows.commitNineKeyLiteral(current.commitText());
    }
}
