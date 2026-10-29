package app.msime.android;

import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.LinearLayout;

/**
 * 大屏单手模式空出来那一侧的数字键盘（#10203）：3 列 × 4 行，与电话拨号盘相同（1 2 3 / 4 5 6 / 7 8 9 / . 0 ⌫）。摆在哪、什么时候摆由 {@link ImeFrame#applyOneHanded} 按 {@link OneHandSidePadPolicy#shown} 决定，这里只建面板和面板与侧栏之间的空白。
 *
 * <p>面板是一个独立的 {@link KeyboardKeyArea}：键距是键的外边距，落在键与键之间空隙里的按下交给最近的键，和主键区一样。前三行是一块九宫格高度的键块（{@link MSIMEInputService.KeyboardHeightRole#nineKeyGrid}），第四行是底行高度（{@link MSIMEInputService.KeyboardHeightRole#bottomRow}），两块都按主键区同一套高度角色定高，整块贴着行的底边，于是各行的上下边与主键区的九键网格和底行对齐。键距、行高经 {@link ImeStyler#applySidePadGeometry} 跟着主键区一起重排，配色由换肤时的整树上色带上。
 *
 * <p>数字和 `.` 走数字层的字面上屏（{@link ImeLayoutRows#commitNineKeyLiteral}）：正在组的拼音先原样上屏，再上屏这个字符；⌫ 与九键右列的 ⌫ 同一个动作，带连删。不挂滑行、横滑移动光标、长按选项和上滑角标。
 */
final class ImeSidePad {
    /** 面板根视图的 tag，测试和排查时按它找面板。 */
    static final String TAG = "oneHandSidePad";
    private static final String[][] DIGIT_ROWS = {{"1", "2", "3"}, {"4", "5", "6"}, {"7", "8", "9"}};

    private final MSIMEInputService s;
    private KeyboardKeyArea pad;
    private View spacer;

    ImeSidePad(MSIMEInputService s) {
        this.s = s;
    }

    /** 已经建好的面板；还没用到过时为 null。 */
    KeyboardKeyArea built() {
        return pad;
    }

    /** 面板，第一次用到时建。 */
    KeyboardKeyArea pad() {
        if (pad == null) pad = build();
        return pad;
    }

    /** 面板与侧栏之间的空白：普通的 View，不可点击、不带键距外边距，{@link KeyboardKeyArea} 不会把落在这里的按下交给任何键，读屏也跳过它。 */
    View spacer() {
        if (spacer == null) {
            spacer = new View(s);
            spacer.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        }
        return spacer;
    }

    /** 把面板和空白从 `row` 里摘下；建好的面板留着，下次摆上时直接用。 */
    void detachFrom(ViewGroup row) {
        if (pad != null && pad.getParent() == row) row.removeView(pad);
        if (spacer != null && spacer.getParent() == row) row.removeView(spacer);
    }

    /** 键盘视图换了一套（onCreateInputView 建了新的键区行）：旧面板留在旧行里，下次用到时重建。 */
    void forget() {
        pad = null;
        spacer = null;
    }

    private KeyboardKeyArea build() {
        Context context = s;
        KeyboardKeyArea area = new KeyboardKeyArea(context, s::followsKeySpacing);
        area.setOrientation(LinearLayout.VERTICAL);
        area.setGravity(Gravity.BOTTOM);
        area.setTag(TAG);
        area.setContentDescription("侧边数字键盘");

        LinearLayout grid = LayoutPolicy.column(context);
        grid.setTag(MSIMEInputService.KeyboardHeightRole.nineKeyGrid());
        for (String[] digits : DIGIT_ROWS) {
            LinearLayout row = LayoutPolicy.row(context);
            for (String digit : digits) s.imeLayoutRows.addNineKey(row, digitKey(digit));
            grid.addView(row, LayoutPolicy.weightedWidthParams(1));
        }
        area.addView(grid, LayoutPolicy.matchWidthHeightPx(s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3)));

        LinearLayout bottom = LayoutPolicy.row(context);
        bottom.setTag(MSIMEInputService.KeyboardHeightRole.bottomRow());
        s.imeLayoutRows.addNineKey(bottom, s.imeLayoutRows.nineKeyDecimalKey());
        s.imeLayoutRows.addNineKey(bottom, digitKey("0"));
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        s.imeLayoutRows.addNineKey(bottom, delete);
        area.addView(bottom, LayoutPolicy.matchWidthHeightPx(s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP)));

        s.imeStyler.applySidePadGeometry(area);
        return area;
    }

    /** 数字键：读屏「按键 数字 N」，与数字层的数字键同一个字号，记普通数字键。 */
    private Button digitKey(String digit) {
        KeyboardPressButton key = s.keyboardKey(digit, "数字 " + digit,
            () -> s.imeLayoutRows.commitNineKeyLiteral(digit));
        KeyboardTypographyPolicy.setFittedKeyTextSize(key, 14, 22);
        key.setKeyboardRole(KeyboardKeyRole.KEY);
        return s.keyId(key, KeyPressIds.forCharacter(digit.charAt(0)));
    }
}
