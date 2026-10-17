package app.msime.android.home;

import android.content.Context;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.fragment.app.Fragment;
import app.msime.android.DimensionPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.NineKeySidebarPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** 九键左侧符号栏的可视化编辑面板：符号逐项列出，顺序和删除动作不靠编辑一串文本完成。 */
final class NineKeySidebarSheet {
    private final Fragment owner;
    private final String key;
    private final List<String> defaults;
    private final SettingsSheet sheet;
    private final LinearLayout rows;
    private final GroupCard.Row resetRow;
    private final GroupCard.Row saveRow;
    private final Runnable saved;
    private List<String> symbols;
    private boolean saving;

    NineKeySidebarSheet(Fragment owner, String title, String key, List<String> current,
            List<String> defaults, Runnable saved) {
        this.owner = owner;
        this.key = key;
        this.defaults = List.copyOf(defaults);
        this.symbols = new ArrayList<>(current);
        this.saved = saved;
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, title, "按键从上到下显示；用箭头调整顺序，删除或添加单个符号。保存后应用到键盘。");
        rows = LayoutPolicy.column(context);
        sheet.add(rows);
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认符号", null, "恢复默认", () -> {
            symbols = new ArrayList<>(this.defaults);
            render();
        });
        actions.button("添加自定义符号", "每次添加一个，最多 " + NineKeySidebarPolicy.MAX_SYMBOLS + " 个",
            "添加", this::addSymbol);
        saveRow = actions.button("保存符号栏", null, "保存", this::save);
        actions.button("放弃本次调整", null, "取消", sheet::dismiss);
        render();
    }

    void show() { sheet.show(); }

    private void render() {
        rows.removeAllViews();
        Context context = owner.requireContext();
        if (symbols.isEmpty()) {
            TextView empty = ViewPolicy.styledLabel(context, "当前没有符号，保存后恢复默认符号。",
                Ui.TEXT_ROW_SUBTITLE, 400, ThemeColorPolicy.subText(context));
            ViewPolicy.setPaddingDp(context, empty, 16, 12, 16, 12);
            rows.addView(empty, LayoutPolicy.matchWidthWrapParams());
        } else {
            for (int index = 0; index < symbols.size(); index++) addRow(index);
        }
        resetRow.setEnabled(!saving);
        saveRow.setEnabled(!saving);
    }

    private void addRow(int index) {
        Context context = owner.requireContext();
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(row);
        TextView symbol = ViewPolicy.styledLabel(context, symbols.get(index), Ui.TEXT_ROW_TITLE, 400,
            ThemeColorPolicy.text(context));
        symbol.setContentDescription("第 " + (index + 1) + " 个符号：" + symbols.get(index));
        row.addView(symbol, LayoutPolicy.weightedWrapParams(1));
        row.addView(actionButton("↑", "上移 " + symbols.get(index), index > 0,
            () -> move(index, -1)), LayoutPolicy.wrapParams());
        row.addView(actionButton("↓", "下移 " + symbols.get(index), index + 1 < symbols.size(),
            () -> move(index, 1)), LayoutPolicy.wrapParams());
        row.addView(actionButton("删除", "删除 " + symbols.get(index), true,
            () -> remove(index)), LayoutPolicy.wrapParams());
        rows.addView(row, LayoutPolicy.matchWidthHeightDp(context, 56));
    }

    private TextView actionButton(String label, String description, boolean enabled, Runnable action) {
        Context context = owner.requireContext();
        TextView button = HomeButtonPolicy.tonalButton(context, label, description, 500);
        button.setMinWidth(DimensionPolicy.pixels(context, label.length() > 1 ? 56 : 48));
        ViewPolicy.setEnabledWithAlpha(button, enabled && !saving, 0.38f);
        ViewPolicy.bindClick(button, action);
        return button;
    }

    private void move(int index, int delta) {
        if (saving) return;
        int target = index + delta;
        if (target < 0 || target >= symbols.size()) return;
        Collections.swap(symbols, index, target);
        render();
    }

    private void remove(int index) {
        if (saving) return;
        symbols.remove(index);
        render();
    }

    private void addSymbol() {
        if (saving || symbols.size() >= NineKeySidebarPolicy.MAX_SYMBOLS) return;
        InputDialog dialog = new InputDialog(owner.requireContext(), "添加自定义符号",
            "一次添加一个符号，最多 " + NineKeySidebarPolicy.MAX_SYMBOL_CODE_POINTS + " 个字符");
        dialog.addField("例如 ； 或 ……", null, 0);
        dialog.setValidator(values -> {
            List<String> parsed = NineKeySidebarPolicy.parse(values.get(0));
            return parsed != null && parsed.size() == 1 && !symbols.contains(parsed.get(0));
        });
        dialog.setPrimary("添加", values -> {
            symbols.add(NineKeySidebarPolicy.parse(values.get(0)).get(0));
            render();
        });
        dialog.show();
    }

    private void save() {
        if (saving) return;
        saving = true;
        render();
        String value = symbols.isEmpty() ? null : NineKeySidebarPolicy.format(symbols);
        KeyboardSheets.saveLocal(owner, key, value,
            () -> {
                sheet.dismiss();
                saved.run();
            }, () -> {
                saving = false;
                render();
            });
    }
}
