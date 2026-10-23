package app.msime.android.home;

import android.text.InputType;
import android.widget.LinearLayout;
import app.msime.android.ClipboardHistoryStore;

/** 剪贴板分类管理；名称规则、数量限制和持久化由共享 Rust 决定。 */
public final class ClipboardCategoriesPage extends ReloadingDetailPage {
    private ClipboardHistoryStore.Catalog catalog;
    private boolean busy;

    @Override protected boolean shouldReloadWhenVisible() { return !busy; }

    @Override protected void reload() {
        HostTask.run(this, context -> new ClipboardHistoryStore(context.getFilesDir()).catalog(), this::render);
    }

    private void render(ClipboardHistoryStore.Catalog value) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        catalog = value;
        target.removeAllViews();
        GroupCard card = GroupCard.add(target, "手动分类");
        if (value == null) {
            card.note("分类读取失败，请稍后重试");
            card.button("重新读取", null, "重试", this::reload);
            return;
        }
        card.note("最多 " + value.maxCategories() + " 个分类，名称最多 " + value.maxNameCharacters()
            + " 字，不能重复或使用本机、云端、全部记录、未分类这些内置名称。新记录默认未分类；删除分类只把记录归回未分类，不删除正文。分类只保存在本机。");
        for (ClipboardHistoryStore.Category category : value.categories()) {
            card.nav(category.name(), "点击修改分类名称", "改名", () -> edit(category));
            card.button("删除分类「" + category.name() + "」", "记录保留并归回未分类", "删除", () -> remove(category));
        }
        if (value.categories().size() < value.maxCategories())
            card.button("添加分类", null, "添加", () -> edit(null));
        else card.note("分类数量已达上限，可改名或删除已有分类。");
    }

    private void edit(ClipboardHistoryStore.Category category) {
        if (busy || catalog == null) return;
        InputDialog dialog = new InputDialog(requireContext(), category == null ? "添加分类" : "修改分类名称",
            "最多 " + catalog.maxNameCharacters() + " 字，名称不能重复");
        dialog.addField("分类名称", category == null ? "" : category.name(), InputType.TYPE_CLASS_TEXT)
            .setContentDescription("剪贴板分类名称");
        dialog.setValidator(values -> !values.get(0).trim().isEmpty());
        dialog.setPrimary("保存", values -> change(store -> category == null
            ? store.addCategory(values.get(0)) : store.renameCategory(category.id(), values.get(0))));
        dialog.show();
    }

    private void remove(ClipboardHistoryStore.Category category) {
        if (busy) return;
        change(store -> store.removeCategory(category.id()));
    }

    private interface Change { boolean apply(ClipboardHistoryStore store); }

    private void change(Change change) {
        if (busy) return;
        busy = true;
        HostTask.run(this, context -> change.apply(new ClipboardHistoryStore(context.getFilesDir())), ok -> {
            busy = false;
            if (getContext() != null) MsToast.show(requireContext(), Boolean.TRUE.equals(ok)
                ? "分类已更新" : "未能保存，请检查名称长度、重名、数量上限，或重新读取分类");
            reload();
        });
    }

    @Override protected void onContentDestroyed() { busy = false; }
}
