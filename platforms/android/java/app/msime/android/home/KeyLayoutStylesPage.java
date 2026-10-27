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
import app.msime.android.KeyboardScheme;
import app.msime.android.LayoutPolicy;
import app.msime.android.R;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TouchKeyStyle;
import app.msime.android.ViewPolicy;
import java.util.List;
import org.json.JSONObject;

/**
 * 「按键布局」子页（键盘页「布局」组进来）：26 键、14 键与 18 键、9 键三组单选，每个选项下面画这种样式的字母层底行（{@link BottomRowPreview}），最后是没有标题的一组里的「顶部数字行」开关（本版本既没有 26 键也没有 14/18 键时不列）。四项都写共享偏好（`touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_number_row`，{@link TouchKeyStyle}），随账号同步，键盘每秒重读偏好时就地换底行，不打断正在组的字。本版本没有某类键面（例如五笔版没有九键）时不列那一组。
 *
 * <p>页面打开时先把本机旧的「长空格布局」和「数字行」设置迁进共享偏好（{@link KeyLayoutStyleMigration}），显示的就是迁移后的选择。
 */
public class KeyLayoutStylesPage extends ReloadingDetailPage {
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
        AppEdition edition = AppEdition.current();
        TouchKeyStyle.Styles styles = TouchKeyStyle.Styles.parse(
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.FOURTEEN_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NINE_KEY_PREFERENCE)));
        boolean twentySix = offersAny(edition, KeyboardScheme.QUANPIN, KeyboardScheme.XIAOHE, KeyboardScheme.WUBI,
            KeyboardScheme.CANTONESE);
        // 双拼和五笔也有 14 键（双拼还有 18 键），所以本版本有双拼或五笔时同样列出这一组。
        boolean fourteen = offersAny(edition, KeyboardScheme.QUANPIN_FOURTEEN_KEY, KeyboardScheme.QUANPIN_EIGHTEEN_KEY,
            KeyboardScheme.XIAOHE, KeyboardScheme.WUBI);
        if (twentySix)
            styleGroup(target, "26 键", KeyboardLayout.STANDARD_TOUCH_LAYOUT, TouchKeyStyle.TWENTY_SIX_KEY_ROWS,
                styles.twentySix(), TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE);
        if (fourteen)
            styleGroup(target, "14 键与 18 键", KeyboardLayout.FOURTEEN_KEY_LAYOUT, TouchKeyStyle.FOURTEEN_KEY_ROWS,
                styles.fourteen(), TouchKeyStyle.FOURTEEN_KEY_PREFERENCE);
        if (offersAny(edition, KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.NINE_KEY_SHUANGPIN, KeyboardScheme.STROKE))
            styleGroup(target, "9 键", KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, TouchKeyStyle.NINE_KEY_ROWS,
                styles.nine(), TouchKeyStyle.NINE_KEY_PREFERENCE);
        // 顶部数字行只画在 26 键、14 键和 18 键上，本版本两类键面都没有（例如只有九键和笔画）时不列这个开关；它和 iOS、鸿蒙一样放在没有标题的一组里。
        if (!twentySix && !fourteen) return;
        GroupCard numbers = GroupCard.add(target, null);
        numbers.toggle("顶部数字行", "26 键、14 键和 18 键的字母上方多一行 1–0，键盘整体高出一行；手机横屏时放不下，不显示",
            preferences.optBoolean(TouchKeyStyle.NUMBER_ROW_PREFERENCE, false),
            checked -> savePreferences(edited -> edited.put(TouchKeyStyle.NUMBER_ROW_PREFERENCE, checked)));
    }

    private static boolean offersAny(AppEdition edition, KeyboardScheme... schemes) {
        for (KeyboardScheme scheme : schemes) if (scheme.offeredBy(edition)) return true;
        return false;
    }

    /** 一组单选：每个样式一行，标题、选中时的 ✓ 和这种样式的底行预览；点已选中的那一项什么也不做。 */
    private void styleGroup(LinearLayout target, String title, int touchLayout, List<TouchKeyStyle.Row> rows,
            TouchKeyStyle.Row selected, String preferenceKey) {
        GroupCard group = GroupCard.add(target, title).withDividers(16);
        for (TouchKeyStyle.Row row : rows) {
            boolean checked = row == selected;
            group.addView(optionRow(requireContext(), label(touchLayout, row), touchLayout, row, checked,
                () -> { if (!checked) savePreferences(edited -> edited.put(preferenceKey, row.wire)); }));
        }
    }

    /** 选项的名字；九键的「逗号句号在空格两侧」叫「底栏带标点」，因为九键的标点原本在侧栏里。 */
    static String label(int touchLayout, TouchKeyStyle.Row row) {
        return switch (row) {
            case STANDARD -> "标准";
            case COMMA_PERIOD -> touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT ? "底栏带标点" : "逗号句号在空格两侧";
            case LONG_SPACE -> "长空格";
            case EMOJI -> "表情键";
        };
    }

    private static View optionRow(Context context, String title, int touchLayout, TouchKeyStyle.Row row,
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
        LinearLayout.LayoutParams previewParams = LayoutPolicy.matchWidthWrapParams();
        previewParams.topMargin = DimensionPolicy.pixels(context, 8);
        column.addView(new BottomRowPreview(context, touchLayout, row), previewParams);
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
