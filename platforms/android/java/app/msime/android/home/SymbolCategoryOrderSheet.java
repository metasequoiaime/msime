package app.msime.android.home;

import android.content.Context;
import androidx.fragment.app.Fragment;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.SymbolCategoryOrderPolicy;

/** 符号分类排序先改面板草稿；保存成功才关闭，取消或滑下关闭不写设置。 */
final class SymbolCategoryOrderSheet {
    private final Fragment owner;
    private final SettingsSheet sheet;
    private final ReorderList<SymbolCategoryOrderPolicy.Item> rows;
    private final Runnable saved;
    private boolean saving;
    private final GroupCard.Row saveRow;
    private final GroupCard.Row resetRow;

    SymbolCategoryOrderSheet(Fragment owner, AndroidLocalSettings.Snapshot settings, Runnable saved) {
        this.owner = owner;
        this.saved = saved;
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, "符号分类顺序", "按住右侧把手拖动调整分类顺序。点击保存后生效。");
        rows = new ReorderList<>(context, SymbolCategoryOrderPolicy.ordered(
            settings.text(AndroidLocalSettings.SYMBOL_CATEGORY_ORDER)), item -> item.label);
        sheet.add(rows.view());
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认顺序", null, "重置", () -> {
            rows.setItems(SymbolCategoryOrderPolicy.DEFAULT_ORDER);
        });
        saveRow = actions.button("保存符号分类顺序", null, "保存", this::save);
        actions.button("放弃本次调整", null, "取消", sheet::dismiss);
    }

    void show() { sheet.show(); }

    private void save() {
        if (saving) return;
        saving = true;
        saveRow.setEnabled(false);
        resetRow.setEnabled(false);
        rows.setLocked(true);
        KeyboardSheets.saveLocal(owner, AndroidLocalSettings.SYMBOL_CATEGORY_ORDER,
            SymbolCategoryOrderPolicy.format(rows.items()),
            () -> { sheet.dismiss(); saved.run(); },
            () -> {
                saving = false;
                saveRow.setEnabled(true);
                resetRow.setEnabled(true);
                rows.setLocked(false);
            });
    }
}
