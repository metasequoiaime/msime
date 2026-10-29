package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.widget.Button;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.AccessibilityDelegateCompat;
import androidx.core.view.ViewCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat;
import app.msime.android.AppEdition;
import app.msime.android.DimensionPolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.KeyLayoutStyleMigration;
import app.msime.android.KeyboardLayout;
import app.msime.android.LayoutPolicy;
import app.msime.android.R;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TouchKeyStyle;
import app.msime.android.ViewPolicy;
import java.util.List;
import org.json.JSONObject;

/**
 * 「按键布局」子页（键盘页「布局」组进来）：26 键、14 键与 18 键、9 键三组单选，每个选项下面画这种样式的字母层底行（{@link BottomRowPreview}，右侧大回车画外框最后两行），每组卡片下方一行说明；26 键一组末尾是「26 键「符」与「123」互换」开关；接着是「数字键盘」一组单选（空格在右列、在 0 右边、在 0 左边），最后是没有标题的一组里的「顶部数字行」开关（本版本既没有 26 键也没有 14/18 键时不列）。各项都写共享偏好（`touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_twenty_six_symbol_first`、`touch_number_pad_space`、`touch_number_row`，{@link TouchKeyStyle}），随账号同步，键盘每秒重读偏好时就地换底行，不打断正在组的字。本版本没有某类键面（例如五笔版没有九键）时不列那一组，三组都没有时键盘页不列入口（{@link TouchKeyStyle.Groups}）。
 *
 * <p>页面打开时先把本机旧的「长空格布局」和「数字行」设置迁进共享偏好（{@link KeyLayoutStyleMigration}），显示的就是迁移后的选择。
 */
public class KeyLayoutStylesPage extends ReloadingDetailPage {
    // 各组下面的说明与「顶部数字行」的说明，iOS、鸿蒙写的是同样的文字。
    static final String TWENTY_SIX_KEY_NOTE = "长空格：符号键移到工具栏；表情键：长按 123 打开符号。";
    static final String FOURTEEN_KEY_NOTE = "表情键：长按 123 打开符号。";
    static final String NINE_KEY_NOTE = "右侧大回车：回车竖占两行，符号键在左下角。";
    static final String NUMBER_ROW_NOTE = "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。";

    @Override protected void reload() {
        HostTask.run(this, context -> {
            KeyLayoutStyleMigration.migrate(context, HostStore.directory(context));
            return KeyboardSheets.preferences(context);
        }, this::render);
    }

    private void render(@Nullable JSONObject preferences) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        if (preferences == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        TouchKeyStyle.Styles styles = TouchKeyStyle.Styles.parse(
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.FOURTEEN_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NINE_KEY_PREFERENCE)));
        // 本版本提供哪几组由 TouchKeyStyle.Groups 决定（双拼和五笔也有 14 键，所以有双拼或五笔时同样列出 14 键与 18 键一组），键盘页的入口按同一条规则显示。每组下面一行说明这组样式里不显而易见的改动，三端文字相同。
        TouchKeyStyle.Groups groups = TouchKeyStyle.Groups.offeredBy(AppEdition.current());
        boolean symbolFirst = preferences.optBoolean(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, false);
        if (groups.twentySix()) {
            GroupCard twentySix = styleGroup(target, "26 键", KeyboardLayout.STANDARD_TOUCH_LAYOUT,
                TouchKeyStyle.TWENTY_SIX_KEY_ROWS, styles.twentySix(), TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE,
                TWENTY_SIX_KEY_NOTE, symbolFirst);
            // 「符」与「123」互换放在 26 键这组里，小图跟着它画（styleGroup 传入的 symbolFirst）。
            twentySix.toggle(TouchKeyStyle.SYMBOL_FIRST_TITLE, TouchKeyStyle.SYMBOL_FIRST_NOTE, symbolFirst,
                checked -> savePreferences(edited -> edited.put(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, checked)));
        }
        if (groups.fourteen())
            styleGroup(target, "14 键与 18 键", KeyboardLayout.FOURTEEN_KEY_LAYOUT, TouchKeyStyle.FOURTEEN_KEY_ROWS,
                styles.fourteen(), TouchKeyStyle.FOURTEEN_KEY_PREFERENCE, FOURTEEN_KEY_NOTE);
        if (groups.nine())
            styleGroup(target, "9 键", KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, TouchKeyStyle.NINE_KEY_ROWS,
                styles.nine(), TouchKeyStyle.NINE_KEY_PREFERENCE, NINE_KEY_NOTE);
        // 九宫格数字键盘三端都从各类中文键面进得去（自己的数字层或借来的数字九宫格），有入口就列这一组。没选过时勾 Android 原来的「空格在右列」，点哪一项写哪一项的写法。
        TouchKeyStyle.NumberPadSpace space = TouchKeyStyle.NumberPadSpace.parse(
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE))).resolved();
        GroupCard numberPad = GroupCard.add(target, TouchKeyStyle.NUMBER_PAD_SPACE_GROUP).withDividers(16);
        for (TouchKeyStyle.NumberPadSpace choice : TouchKeyStyle.NumberPadSpace.CHOICES) {
            boolean checked = choice == space;
            numberPad.addView(optionRow(requireContext(), choice.title, null, checked,
                () -> { if (!checked) savePreferences(edited -> edited.put(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE, choice.wire)); }));
        }
        numberPad.footer(TouchKeyStyle.NUMBER_PAD_SPACE_NOTE);
        // 顶部数字行只画在 26 键、14 键和 18 键上，本版本两类键面都没有（例如只有九键和笔画）时不列这个开关；它和 iOS、鸿蒙一样放在没有标题的一组里。
        if (!groups.numberRow()) return;
        GroupCard numbers = GroupCard.add(target, null);
        numbers.toggle("顶部数字行", NUMBER_ROW_NOTE,
            preferences.optBoolean(TouchKeyStyle.NUMBER_ROW_PREFERENCE, false),
            checked -> savePreferences(edited -> edited.put(TouchKeyStyle.NUMBER_ROW_PREFERENCE, checked)));
    }

    /** 一组单选：每个样式一行，标题、选中时的 ✓ 和这种样式的底行预览，卡片下方一行说明；点已选中的那一项什么也不做。 */
    private GroupCard styleGroup(LinearLayout target, String title, int touchLayout, List<TouchKeyStyle.Row> rows,
            TouchKeyStyle.Row selected, String preferenceKey, String note) {
        return styleGroup(target, title, touchLayout, rows, selected, preferenceKey, note, false);
    }

    private GroupCard styleGroup(LinearLayout target, String title, int touchLayout, List<TouchKeyStyle.Row> rows,
            TouchKeyStyle.Row selected, String preferenceKey, String note, boolean symbolFirst) {
        GroupCard group = GroupCard.add(target, title).withDividers(16);
        for (TouchKeyStyle.Row row : rows) {
            boolean checked = row == selected;
            group.addView(optionRow(requireContext(), label(touchLayout, row),
                new BottomRowPreview(requireContext(), touchLayout, row, symbolFirst), checked,
                () -> { if (!checked) savePreferences(edited -> edited.put(preferenceKey, row.wire)); }));
        }
        group.footer(note);
        return group;
    }

    /** 选项的名字；九键的「逗号句号在空格两侧」叫「底栏带标点」，因为九键的标点原本在侧栏里；「右侧大回车」只有九键有。 */
    static String label(int touchLayout, TouchKeyStyle.Row row) {
        return switch (row) {
            case STANDARD -> "标准";
            case COMMA_PERIOD -> touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT ? "底栏带标点" : "逗号句号在空格两侧";
            case LONG_SPACE -> "长空格";
            case EMOJI -> "表情键";
            case TALL_RETURN -> "右侧大回车";
        };
    }

    /** 单选的一行：标题、选中时的 ✓，`preview` 不为空时画在标题下面。 */
    private static View optionRow(Context context, String title, @Nullable View preview,
            boolean checked, Runnable action) {
        LinearLayout column = LayoutPolicy.column(context);
        ViewPolicy.setRowPadding(context, column);
        ViewPolicy.makeClickable(column, context, action);
        LinearLayout header = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(header);
        TextView name = ViewPolicy.styledLabel(context, title, Ui.TEXT_ROW_TITLE, checked ? 600 : 400,
            checked ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.text(context));
        header.addView(name, LayoutPolicy.weightedWrapParams(1f));
        if (checked) {
            ImageView check = ImageViewPolicy.decorative(context, R.drawable.ms_w1_a2_check,
                ThemeColorPolicy.accent(context));
            header.addView(check, LayoutPolicy.squareParams(context, Ui.SHEET_CHECK_SIZE));
        }
        column.addView(header, LayoutPolicy.matchWidthWrapParams());
        if (preview != null) {
            LinearLayout.LayoutParams previewParams = LayoutPolicy.matchWidthWrapParams();
            previewParams.topMargin = DimensionPolicy.pixels(context, 8);
            column.addView(preview, previewParams);
        }
        ViewCompat.setAccessibilityDelegate(column, new AccessibilityDelegateCompat() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfoCompat info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Button.class.getName());
                info.setContentDescription(title);
                if (checked) info.setStateDescription("已选择");
            }
        });
        return column;
    }
}
