package app.msime.android.home;

import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;

/** Owns the content column for a detail page's current view. */
public abstract class ColumnDetailPage extends DetailPage {
    @Nullable private LinearLayout column;

    @Override protected final void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        onColumnReady(args);
    }

    protected abstract void onColumnReady(Bundle args);

    @Override public void onDestroyView() {
        column = null;
        onContentDestroyed();
        super.onDestroyView();
    }

    /** Releases state tied to the current view. */
    protected void onContentDestroyed() {}

    @Nullable protected final LinearLayout contentColumn() {
        return column;
    }
}
