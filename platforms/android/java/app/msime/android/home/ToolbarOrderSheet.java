package app.msime.android.home;

import android.content.Context;
import androidx.fragment.app.Fragment;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.ToolbarOrderPolicy;

/** 排序先改面板草稿；保存成功才关闭，取消或滑下关闭不写设置。拖动和读屏「上移」「下移」由 {@link ReorderList} 管，与其余排序面板相同（#10546）。 */
final class ToolbarOrderSheet {
    private final Fragment owner;
    private final SettingsSheet sheet;
    private final ReorderList<ToolbarOrderPolicy.Item> rows;
    private final Runnable saved;
    private boolean saving;
    private final GroupCard.Row saveRow;
    private final GroupCard.Row resetRow;

    ToolbarOrderSheet(Fragment owner, AndroidLocalSettings.Snapshot settings, Runnable saved) {
        this.owner = owner;
        this.saved = saved;
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, "按钮顺序", "按住右侧把手拖动调整顺序，已隐藏的按钮会保留位置。点击保存后生效。");
        rows = new ReorderList<>(context, ToolbarOrderPolicy.ordered(
            settings.text(AndroidLocalSettings.TOOLBAR_ORDER)), item -> item.label);
        sheet.add(rows.view());
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认顺序", null, "重置", () -> {
            if (saving) return;
            rows.setItems(ToolbarOrderPolicy.DEFAULT_ORDER);
        });
        saveRow = actions.button("保存按钮顺序", null, "保存", this::save);
        actions.button("放弃本次调整", null, "取消", sheet::dismiss);
    }

    void show() { sheet.show(); }

    private void save() {
        if (saving) return;
        saving = true;
        saveRow.setEnabled(false);
        resetRow.setEnabled(false);
        rows.setLocked(true);
        KeyboardSheets.saveLocal(owner, AndroidLocalSettings.TOOLBAR_ORDER, ToolbarOrderPolicy.format(rows.items()),
            () -> { sheet.dismiss(); saved.run(); },
            () -> {
                saving = false;
                saveRow.setEnabled(true);
                resetRow.setEnabled(true);
                rows.setLocked(false);
            });
    }
}
