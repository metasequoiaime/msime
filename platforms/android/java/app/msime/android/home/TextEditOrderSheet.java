package app.msime.android.home;

import android.content.Context;
import androidx.fragment.app.Fragment;
import org.json.JSONObject;
import app.msime.android.TextEditButtonOrder;
import app.msime.android.TextEditPanelModel;
import app.msime.android.TextEditPanelModel.Action;
import java.util.List;

/** 排序先改面板草稿；保存成功才关闭，取消或滑下关闭不写设置。 */
final class TextEditOrderSheet {
    private final Fragment owner;
    private final SettingsSheet sheet;
    private final ReorderList<Action> rows;
    private final Runnable saved;
    private boolean saving;
    private final GroupCard.Row saveRow;
    private final GroupCard.Row resetRow;
    private final GroupCard.Row cancelRow;

    TextEditOrderSheet(Fragment owner, JSONObject preferences, Runnable saved) {
        this.owner = owner;
        this.saved = saved;
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, "文本编辑按键顺序", "位置 1–4 是左列从上到下，5–7 是右列前三格，第 8 个在左下方；换行固定在右下角。按住右侧把手拖动调整，点击保存后生效。");
        rows = new ReorderList<>(context, TextEditButtonOrder.fromPreferences(preferences),
            item -> TextEditPanelModel.key(item).label());
        sheet.add(rows.view());
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认顺序", null, "重置", () -> {
            if (saving) return;
            rows.setItems(TextEditButtonOrder.defaults());
        });
        saveRow = actions.button("保存按钮顺序", null, "保存", this::save);
        cancelRow = actions.button("放弃本次调整", null, "取消", sheet::dismiss);
    }

    void show() { sheet.show(); }

    private void save() {
        if (saving) return;
        saving = true;
        saveRow.setEnabled(false);
        resetRow.setEnabled(false);
        cancelRow.setEnabled(false);
        rows.setLocked(true);
        List<Action> order = rows.items();
        KeyboardSheets.save(owner, preferences -> preferences.put(TextEditPanelModel.ORDER_PREFERENCE,
                TextEditButtonOrder.encode(order)),
            () -> { sheet.dismiss(); saved.run(); },
            () -> {
                saving = false;
                saveRow.setEnabled(true);
                resetRow.setEnabled(true);
                cancelRow.setEnabled(true);
                rows.setLocked(false);
            });
    }
}
