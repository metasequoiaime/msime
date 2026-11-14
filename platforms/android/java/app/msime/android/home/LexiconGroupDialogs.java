package app.msime.android.home;

import android.content.Context;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.ScreenCaptureProtection;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.util.ArrayList;
import java.util.List;

/** 命名词库分组的多选管理和导入目标，只处理界面，不复制集合规则。 */
final class LexiconGroupDialogs {
    interface ImportTarget {
        void selected(String id, String name);
    }

    private LexiconGroupDialogs() {}

    static String sourceTitle(String source) {
        return switch (source) {
            case "import" -> "文件导入";
            case "community" -> "社区词库";
            default -> "自建词库";
        };
    }

    static void manage(LexiconPage owner, DictionaryCollectionsStore.View view, Runnable refresh) {
        Context context = owner.requireContext();
        List<DictionaryCollectionsStore.Collection> collections = view.collections();
        if (collections.isEmpty()) {
            MsToast.show(context, "还没有可管理的分组");
            return;
        }
        String[] labels = new String[collections.size()];
        boolean[] selected = new boolean[collections.size()];
        for (int i = 0; i < labels.length; i++) {
            var item = collections.get(i);
            labels[i] = item.name() + " · " + sourceTitle(item.sourceType());
        }
        ScreenCaptureProtection.show(new MaterialAlertDialogBuilder(context)
            .setTitle("选择词库")
            .setMultiChoiceItems(labels, selected, (dialog, index, checked) -> selected[index] = checked)
            .setNegativeButton("取消", null)
            .setPositiveButton("管理所选", (dialog, which) -> {
                List<String> ids = new ArrayList<>();
                for (int i = 0; i < selected.length; i++) if (selected[i]) ids.add(collections.get(i).id());
                if (ids.isEmpty()) {
                    MsToast.show(context, "请先选择词库");
                    return;
                }
                OptionSheet actions = new OptionSheet(context, "已选 " + ids.size() + " 个词库", null);
                actions.option("启用", false, () -> change(owner, ids, true, refresh));
                actions.option("停用", false, () -> change(owner, ids, false, refresh));
                actions.destructive("删除", () -> MessageDialog.confirm(context, "删除所选词库？",
                    "删除这 " + ids.size() + " 个分组及其词条；其他已启用分组和原有个人词条会保留。",
                    "删除", () -> change(owner, ids, null, refresh)));
                actions.show();
            }).create());
    }

    private static void change(LexiconPage owner, List<String> ids, Boolean enabled, Runnable refresh) {
        HostTask.run(owner, context -> DictionaryCollectionsStore.changeSelected(context, ids, enabled), result -> {
            MsToast.show(owner.requireContext(), result != null && result.ok() ? "已更新所选词库"
                : result == null ? DictionaryCollectionsStore.failureMessage("") : result.failure());
            refresh.run();
        });
    }

    static void importTarget(Context context, DictionaryCollectionsStore.View view, String format,
                             String suggestedName, ImportTarget target) {
        OptionSheet choices = new OptionSheet(context, "导入到", null);
        choices.option("新建分组", false, () -> {
            InputDialog naming = new InputDialog(context, "分组名称", null);
            naming.addField("分组名称", suggestedName, 0);
            naming.setValidator(values -> DictionaryCollectionsStore.validName(values.get(0)));
            naming.setPrimary("导入", values -> target.selected(null, values.get(0)));
            naming.show();
        });
        for (var item : view.collections()) {
            if (!"pinyin".equals(item.kind()) || "community".equals(item.sourceType())) continue;
            choices.option(item.name(), false, () -> target.selected(item.id(), item.name()));
        }
        choices.show();
    }
}
