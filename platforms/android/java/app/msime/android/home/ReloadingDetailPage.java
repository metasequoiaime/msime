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

    protected abstract void reload();
}
