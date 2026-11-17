package app.msime.android.home;

import android.content.Context;
import androidx.fragment.app.Fragment;
import app.msime.android.policy.PluginCatalogPolicy;
import java.util.ArrayList;
import java.util.List;

/**
 * 「我的插件」里一个类型分组的排序面板：拖动先改面板草稿，保存成功才关闭，取消或滑下关闭不写设置。拖动、读屏「上移」「下移」和 Alt+↑ / Alt+↓ 由 {@link ReorderList} 管，与其余排序面板相同。
 *
 * <p>保存写共享偏好 `plugins.plugin_order`（{@link PluginStore#reorderGroup}）：只换这一组占着的位置，别的组不动。这份顺序只决定设置页里的排列，短语表、指令表的生效先后仍是启用顺序。
 */
final class PluginOrderSheet {
    private final Fragment owner;
    private final SettingsSheet sheet;
    private final ReorderList<PluginCatalogPolicy.Pack> rows;
    private final List<PluginCatalogPolicy.Pack> packs;
    private final Runnable saved;
    private boolean saving;
    private final GroupCard.Row saveRow;
    private final GroupCard.Row resetRow;

    /**
     * @param packs 目录里所有的包，按目录原来的先后
     * @param order 偏好里现在的 `plugins.plugin_order`
     */
    PluginOrderSheet(Fragment owner, String kind, List<PluginCatalogPolicy.Pack> packs, List<String> order, Runnable saved) {
        this.owner = owner;
        this.packs = List.copyOf(packs);
        this.saved = saved;
        Context context = owner.requireContext();
        boolean table = PluginCatalogPolicy.COMMAND_TABLE.equals(kind) || PluginCatalogPolicy.PHRASE_TABLE.equals(kind);
        sheet = new SettingsSheet(context, PluginCatalogPolicy.kindLabel(kind) + "顺序",
            table ? "按住右侧把手拖动调整列表里的排列，生效的先后仍按启用顺序。点击保存后生效。" : "按住右侧把手拖动调整列表里的排列。点击保存后生效。");
        rows = new ReorderList<>(context, PluginCatalogPolicy.groupInOrder(packs, kind, order), PluginCatalogPolicy.Pack::name);
        sheet.add(rows.view());
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认顺序", null, "重置", () -> {
            if (saving) return;
            rows.setItems(PluginCatalogPolicy.groupInOrder(packs, kind, List.of()));
        });
        saveRow = actions.button("保存顺序", null, "保存", this::save);
        actions.button("放弃本次调整", null, "取消", sheet::dismiss);
    }

    void show() { sheet.show(); }

    private void save() {
        if (saving) return;
        saving = true;
        saveRow.setEnabled(false);
        resetRow.setEnabled(false);
        rows.setLocked(true);
        List<String> group = new ArrayList<>();
        for (PluginCatalogPolicy.Pack pack : rows.items()) group.add(PluginCatalogPolicy.pluginKey(pack.kind(), pack.id()));
        KeyboardSheets.save(owner, preferences -> PluginStore.reorderGroup(preferences, packs, group),
            () -> { sheet.dismiss(); saved.run(); },
            () -> {
                saving = false;
                saveRow.setEnabled(true);
                resetRow.setEnabled(true);
                rows.setLocked(false);
            });
    }
}
