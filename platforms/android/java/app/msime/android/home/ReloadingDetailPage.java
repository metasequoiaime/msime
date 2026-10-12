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

    @Override protected void onBecameVisible() {
        if (contentColumn() != null && shouldReloadWhenVisible()) reload();
    }

    /** Allows a page to preserve active editing state when it becomes visible again. */
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
