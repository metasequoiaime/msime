package app.msime.android.home;

import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;

/** 在首次构建和每次重新可见时刷新内容的详情页基类。 */
public abstract class ReloadingDetailPage extends DetailPage {
    @Nullable private LinearLayout column;

    @Override protected final void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    @Nullable protected final LinearLayout contentColumn() {
        return column;
    }

    protected abstract void reload();
}
