package app.msime.android.home;

import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import androidx.core.view.AccessibilityDelegateCompat;
import androidx.core.view.ViewCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat;
import androidx.core.widget.NestedScrollView;
import app.msime.android.AppIconStyle;
import app.msime.android.DimensionPolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.R;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import com.google.android.material.bottomsheet.BottomSheetDialog;

/** 主屏图标选择面板：用图标网格展示每种配色，文字只留给无障碍。 */
public final class AppIconSheet {
    private static final int COLUMNS = 2;
    private static final int CELL_HEIGHT = 118;
    private static final int ICON_SIZE = 76;

    private final Context context;
    private final BottomSheetDialog dialog;
    private final LinearLayout rows;
    private LinearLayout currentRow;
    private int count;

    public AppIconSheet(Context context) {
        this.context = context;
        dialog = SheetDialog.create(context);
        LinearLayout root = LayoutPolicy.column(context);
        root.addView(LayoutPolicy.sheetDragHandle(context));
        root.addView(SheetHeaderView.create(context, "App 图标",
            "点图标切换主屏显示的配色"));

        rows = LayoutPolicy.column(context);
        NestedScrollView scroll = new NestedScrollView(context);
        scroll.addView(rows, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        root.addView(scroll, LayoutPolicy.weightedWidthParams(1f));
        root.addView(LayoutPolicy.colorBand(context, ThemeColorPolicy.page(context), 8));
        root.addView(SheetOptionView.create(context, "取消", false, false,
            ThemeColorPolicy.accent(context), true, dialog::cancel));
        dialog.setContentView(root);
    }

    /** 加入一个可点击的图标；标题和描述通过无障碍节点保留。 */
    public AppIconSheet option(AppIconStyle style, boolean selected, Runnable action) {
        if (count % COLUMNS == 0) {
            currentRow = LayoutPolicy.row(context);
            rows.addView(currentRow, LayoutPolicy.matchWidthWrapParams());
        }
        View cell = cell(style, selected, action);
        LinearLayout.LayoutParams params = LayoutPolicy.weightedWrapParams(1f);
        params.setMargins(DimensionPolicy.pixels(context, 4), DimensionPolicy.pixels(context, 2),
            DimensionPolicy.pixels(context, 4), DimensionPolicy.pixels(context, 2));
        currentRow.addView(cell, params);
        count++;
        return this;
    }

    public void show() {
        dialog.show();
    }

    private View cell(AppIconStyle style, boolean selected, Runnable action) {
        FrameLayout cell = new FrameLayout(context);
        ViewPolicy.setMinimumHeightDp(context, cell, CELL_HEIGHT);
        ViewPolicy.makeClickable(cell, context, () -> {
            dialog.dismiss();
            action.run();
        });

        ImageView icon = ImageViewPolicy.decorative(context, iconResource(style));
        icon.setScaleType(ImageView.ScaleType.CENTER_INSIDE);
        cell.addView(icon, LayoutPolicy.squareFrameParamsPx(
            DimensionPolicy.pixels(context, ICON_SIZE), Gravity.CENTER));

        if (selected) {
            ImageView check = ImageViewPolicy.decorative(context, R.drawable.ms_w1_a2_check,
                ThemeColorPolicy.accent(context));
            FrameLayout.LayoutParams checkParams = LayoutPolicy.squareFrameParams(context, Ui.SHEET_CHECK_SIZE);
            checkParams.gravity = Gravity.TOP | Gravity.END;
            checkParams.setMargins(0, DimensionPolicy.pixels(context, 8),
                DimensionPolicy.pixels(context, 8), 0);
            cell.addView(check, checkParams);
        }

        String description = style.title() + "，" + style.description();
        ViewCompat.setAccessibilityDelegate(cell, new AccessibilityDelegateCompat() {
            @Override public void onInitializeAccessibilityNodeInfo(View host,
                    AccessibilityNodeInfoCompat info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName("android.widget.Button");
                info.setContentDescription(description);
                if (selected) info.setStateDescription("已选择");
            }
        });
        cell.setContentDescription(description);
        return cell;
    }

    private static int iconResource(AppIconStyle style) {
        return AppIcons.resource(style);
    }
}
