package app.msime.android.home;

import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;

/** 在首次构建和每次重新可见时刷新内容的详情页基类。 */
public abstract class ReloadingDetailPage extends DetailPage {
    @Nullable private LinearLayout column;

    @Override protected final void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        onContentReady();
        reload();
        onInitialReloadScheduled();
    }

    /** Allows a page to paint retained content before its first asynchronous refresh. */
    protected void onContentReady() {}

    /** Allows a page to start extra work after scheduling its first refresh. */
    protected void onInitialReloadScheduled() {}

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        onContentDestroyed();
        super.onDestroyView();
    }

    /** Allows a page to release state tied to the current view. */
    protected void onContentDestroyed() {}

    @Nullable protected final LinearLayout contentColumn() {
        return column;
    }

    protected abstract void reload();
}
