package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.fragment.app.Fragment;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.DimensionPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.SymbolCategoryOrderPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import java.util.List;

/** 符号分类排序先改面板草稿；保存成功才关闭，取消或滑下关闭不写设置。 */
final class SymbolCategoryOrderSheet {
    private final Fragment owner;
    private final SettingsSheet sheet;
    private final LinearLayout rows;
    private final Runnable saved;
    private List<SymbolCategoryOrderPolicy.Item> order;
    private boolean saving;
    private final GroupCard.Row saveRow;
    private final GroupCard.Row resetRow;

    SymbolCategoryOrderSheet(Fragment owner, AndroidLocalSettings.Snapshot settings, Runnable saved) {
        this.owner = owner;
        this.saved = saved;
        this.order = SymbolCategoryOrderPolicy.ordered(
            settings.text(AndroidLocalSettings.SYMBOL_CATEGORY_ORDER));
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, "符号分类顺序", "用上下箭头调整分类顺序。点击保存后生效。");
        rows = LayoutPolicy.column(context);
        sheet.add(rows);
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认顺序", null, "重置", () -> {
            order = SymbolCategoryOrderPolicy.DEFAULT_ORDER;
            render();
        });
        saveRow = actions.button("保存符号分类顺序", null, "保存", this::save);
        actions.button("放弃本次调整", null, "取消", sheet::dismiss);
        render();
    }

    void show() { sheet.show(); }

    private void render() {
        rows.removeAllViews();
        Context context = owner.requireContext();
        for (int index = 0; index < order.size(); index++) {
            SymbolCategoryOrderPolicy.Item item = order.get(index);
            LinearLayout row = LayoutPolicy.row(context);
            ViewPolicy.setCenteredVertically(row);
            TextView title = ViewPolicy.styledLabel(context, (index + 1) + ". " + item.label,
                Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
            row.addView(title, LayoutPolicy.weightedWrapParams(1));
            addArrow(row, item, -1, index > 0);
            addArrow(row, item, 1, index + 1 < order.size());
            rows.addView(row, LayoutPolicy.matchWidthHeightDp(context, 48));
        }
    }

    private void addArrow(LinearLayout row, SymbolCategoryOrderPolicy.Item item, int delta, boolean enabled) {
        Context context = owner.requireContext();
        TextView button = ViewPolicy.styledLabel(context, delta < 0 ? "↑" : "↓", 22, 500,
            ThemeColorPolicy.accent(context));
        ViewPolicy.setCentered(button);
        button.setContentDescription((delta < 0 ? "上移 " : "下移 ") + item.label);
        button.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View view,
                    android.view.accessibility.AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(view, info);
                info.setClassName(android.widget.Button.class.getName());
            }
        });
        ViewPolicy.setBackground(button, ViewPolicy.ripple(context));
        ViewPolicy.setEnabledWithAlpha(button, enabled && !saving, 0.38f);
        ViewPolicy.bindClick(button, () -> {
            if (saving) return;
            order = SymbolCategoryOrderPolicy.move(order, item, delta);
            render();
        });
        row.addView(button, new LinearLayout.LayoutParams(DimensionPolicy.pixels(context, 48),
            LinearLayout.LayoutParams.MATCH_PARENT));
    }

    private void save() {
        if (saving) return;
        saving = true;
        saveRow.setEnabled(false);
        resetRow.setEnabled(false);
        render();
        KeyboardSheets.saveLocal(owner, AndroidLocalSettings.SYMBOL_CATEGORY_ORDER,
            SymbolCategoryOrderPolicy.format(order),
            () -> { sheet.dismiss(); saved.run(); },
            () -> {
                saving = false;
                saveRow.setEnabled(true);
                resetRow.setEnabled(true);
                render();
            });
    }
}
