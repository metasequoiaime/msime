package app.msime.android.home;

import android.os.Bundle;

/** 在首次构建和每次重新可见时刷新内容的详情页基类。 */
public abstract class ReloadingDetailPage extends ColumnDetailPage {
    @Override protected final void onColumnReady(Bundle args) {
        onContentReady();
        if (shouldReloadOnContentReady()) reload();
        onInitialReloadScheduled();
    }

    /** Allows a page to paint retained content before its first asynchronous refresh. */
    protected void onContentReady() {}

    /** Allows a page to wait for its first visible callback before loading. */
    protected boolean shouldReloadOnContentReady() { return true; }

    /** Allows a page to start extra work after scheduling its first refresh. */
    protected void onInitialReloadScheduled() {}

    @Override protected final void onBecameVisible() {
        if (contentColumn() != null) onVisible();
    }

    /** 页面重新可见时执行刷新或页面自己的前置操作。 */
    protected void onVisible() {
        if (shouldReloadWhenVisible()) reload();
    }

    /** 允许页面在重新可见时保留正在编辑的状态。 */
    protected boolean shouldReloadWhenVisible() { return true; }

    /** Saves one Android-local setting and refreshes the page after success or failure. */
    protected final void saveLocalSetting(String key, Object value) {
        KeyboardSheets.saveLocal(this, key, value, null, this::reload);
    }

    /** Saves shared preferences and refreshes the page after either outcome. */
    protected final void savePreferences(KeyboardSheets.Edit edit) {
        KeyboardSheets.save(this, edit, this::reload, this::reload);
    }

    /** Saves shared preferences, runs an extra success action, then refreshes the page. */
    protected final void savePreferences(KeyboardSheets.Edit edit, Runnable then) {
        KeyboardSheets.save(this, edit, () -> {
            then.run();
            reload();
        }, this::reload);
    }

    protected abstract void reload();
}
