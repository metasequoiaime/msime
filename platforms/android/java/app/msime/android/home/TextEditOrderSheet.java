package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.fragment.app.Fragment;
import org.json.JSONObject;
import app.msime.android.DimensionPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TextEditButtonOrder;
import app.msime.android.TextEditPanelModel;
import app.msime.android.TextEditPanelModel.Action;
import app.msime.android.ViewPolicy;
import java.util.List;

/** 排序先改面板草稿；保存成功才关闭，取消或滑下关闭不写设置。 */
final class TextEditOrderSheet {
    private final Fragment owner;
    private final SettingsSheet sheet;
    private final LinearLayout rows;
    private final Runnable saved;
    private List<Action> order;
    private boolean saving;
    private final GroupCard.Row saveRow;
    private final GroupCard.Row resetRow;
    private final GroupCard.Row cancelRow;

    TextEditOrderSheet(Fragment owner, JSONObject preferences, Runnable saved) {
        this.owner = owner;
        this.saved = saved;
        this.order = TextEditButtonOrder.fromPreferences(preferences);
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, "文本编辑按键顺序", "位置 1–4 是左列从上到下，5–7 是右列前三格；换行固定在右下角。用上下箭头调整，点击保存后生效。");
        rows = LayoutPolicy.column(context);
        sheet.add(rows);
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认顺序", null, "重置", () -> {
            if (saving) return;
            order = TextEditButtonOrder.defaults();
            render();
        });
        saveRow = actions.button("保存按钮顺序", null, "保存", this::save);
        cancelRow = actions.button("放弃本次调整", null, "取消", sheet::dismiss);
        render();
    }

    void show() { sheet.show(); }

    private void render() {
        rows.removeAllViews();
        Context context = owner.requireContext();
        for (int index = 0; index < order.size(); index++) {
            Action item = order.get(index);
            LinearLayout row = LayoutPolicy.row(context);
            ViewPolicy.setCenteredVertically(row);
            TextView title = ViewPolicy.styledLabel(context, (index + 1) + ". " + TextEditPanelModel.key(item).label(),
                Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
            row.addView(title, LayoutPolicy.weightedWrapParams(1));
            addArrow(row, item, -1, index > 0);
            addArrow(row, item, 1, index + 1 < order.size());
            rows.addView(row, LayoutPolicy.matchWidthHeightDp(context, 48));
        }
    }

    private void addArrow(LinearLayout row, Action item, int delta, boolean enabled) {
        Context context = owner.requireContext();
        TextView button = ViewPolicy.styledLabel(context, delta < 0 ? "↑" : "↓", 22, 500,
            ThemeColorPolicy.accent(context));
        ViewPolicy.setCentered(button);
        button.setContentDescription((delta < 0 ? "上移 " : "下移 ") + TextEditPanelModel.key(item).label());
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
            java.util.ArrayList<Action> next = new java.util.ArrayList<>(order);
            int index = next.indexOf(item);
            int target = index + delta;
            if (target < 0 || target >= next.size()) return;
            java.util.Collections.swap(next, index, target);
            order = List.copyOf(next);
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
        cancelRow.setEnabled(false);
        render();
        KeyboardSheets.save(owner, preferences -> preferences.put(TextEditPanelModel.ORDER_PREFERENCE,
                TextEditButtonOrder.encode(order)),
            () -> { sheet.dismiss(); saved.run(); },
            () -> {
                saving = false;
                saveRow.setEnabled(true);
                resetRow.setEnabled(true);
                cancelRow.setEnabled(true);
                render();
            });
    }
}
