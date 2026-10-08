package app.msime.android;

import android.app.Dialog;
import android.content.Context;
import android.graphics.Color;
import android.os.Build;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsetsController;
import android.widget.LinearLayout;

/**
 * 包住键区的容器：键行与底行经这里放进键盘的竖向布局，导航栏颜色在这里跟随键盘底色。
 *
 * <p>单手模式（`touch_one_handed` 为 `left` / `right`）时，键行与底行收窄到总宽的 85% 推向那一侧，另一侧是 {@link OneHandGutterView}：‹ 换到另一侧、⤢ 退出单手模式，两者都只写偏好（经 SVC 的 toggleOneHanded）。工具栏与候选条不在这里，保持全宽。`off` 时侧栏不显示，键区占满全宽，与原来相同。
 */
final class ImeFrame {
    private final MSIMEInputService s;
    /** onCreateInputView 里建好的键盘竖向布局，键区放进它。 */
    LinearLayout keyboard;
    private int navigationColor;
    private boolean navigationDark;
    private boolean navigationApplied;
    /** 键行与底行所在的竖向一列；单手模式时收窄。 */
    private LinearLayout column;
    private OneHandRow row;
    private OneHandGutterView gutter;
    private String appliedMode = "";

    ImeFrame(MSIMEInputService s) {
        this.s = s;
    }

    /** 侧栏在哪一边：键盘靠右时在左（索引 0），靠左时在右。 */
    static boolean gutterOnLeft(String mode) {
        return "right".equals(mode);
    }

    /** 偏好值是否开启单手模式。 */
    static boolean oneHanded(String mode) {
        return "left".equals(mode) || "right".equals(mode);
    }

    /** 竖向一列里一个可见的子视图都没有时（回复键盘等盖住键区的面板打开时），整行不占高度，侧栏也不画。 */
    private static final class OneHandRow extends LinearLayout {
        private LinearLayout keys;

        OneHandRow(Context context) {
            super(context);
            setOrientation(HORIZONTAL);
        }

        @Override protected void onMeasure(int widthSpec, int heightSpec) {
            boolean anyVisible = false;
            if (keys != null) {
                for (int index = 0; index < keys.getChildCount(); index++) {
                    if (keys.getChildAt(index).getVisibility() != View.GONE) {
                        anyVisible = true;
                        break;
                    }
                }
            }
            if (!anyVisible) {
                setMeasuredDimension(MeasureSpec.getSize(widthSpec), 0);
                return;
            }
            super.onMeasure(widthSpec, heightSpec);
        }
    }

    private LinearLayout column() {
        if (column != null && row != null && row.getParent() == keyboard) return column;
        row = new OneHandRow(s);
        column = KeyboardGeometry.column(s);
        row.keys = column;
        gutter = new OneHandGutterView(s);
        gutter.setOnSwap(() -> s.toggleOneHanded(true));
        gutter.setOnExit(() -> {
            if (oneHanded(s.oneHandedMode)) s.toggleOneHanded(false);
        });
        ViewPolicy.hide(gutter);
        row.addView(column, KeyboardGeometry.weightedWrapParams(1));
        keyboard.addView(row, KeyboardGeometry.matchWidthWrapParams());
        appliedMode = "";
        return column;
    }

    /** 外接键盘的候选条模式里收起整行键区（{@link HardwareKeyboardModePolicy#keysCollapsed}）；单手侧栏在同一行里，一起收起。 */
    void setKeysCollapsed(boolean collapsed) {
        if (row != null) ViewPolicy.setVisible(row, !collapsed);
    }

    /** 按默认布局参数放入键区。 */
    void wrap(ViewGroup keyArea) {
        column().addView(keyArea, KeyboardGeometry.matchWidthWrapParams());
        applyOneHanded();
    }

    /** 按给定布局参数放入键区。 */
    void wrap(ViewGroup keyArea, ViewGroup.LayoutParams params) {
        column().addView(keyArea, params);
        applyOneHanded();
    }

    /** 按当前 `touch_one_handed` 摆放侧栏与键区；与上次相同时只刷新颜色。渲染与换肤时调用。分离式键盘画着的时候单手模式不生效（存着的值不变，回到不分离时自动恢复），见 {@link SplitKeyboardPolicy#effectiveOneHanded}。 */
    void applyOneHanded() {
        if (row == null || gutter == null || column == null) return;
        // 浮动键盘本身就是一块窄面板，单手模式与分离式键盘一样不生效，存着的值不变。
        String stored = SplitKeyboardPolicy.effectiveOneHanded(s.oneHandedMode,
            s.splitKeyboardDrawn() || s.floatingDrawn());
        String mode = oneHanded(stored) ? stored : "off";
        if (s.skin != null) {
            gutter.setColors(Color.parseColor(s.skin.keyBackground()), Color.parseColor(s.skin.toolbarIcon()));
        }
        if (mode.equals(appliedMode)) return;
        appliedMode = mode;
        if (gutter.getParent() != null) row.removeView(gutter);
        if (!oneHanded(mode)) {
            ViewPolicy.hide(gutter);
            column.setLayoutParams(KeyboardGeometry.weightedWrapParams(1));
            row.requestLayout();
            return;
        }
        float gutterWeight = OneHandGutterView.GUTTER_FRACTION;
        LinearLayout.LayoutParams gutterParams = KeyboardGeometry.weightedMatchParentParams(gutterWeight);
        boolean left = gutterOnLeft(mode);
        gutter.setKeyboardOnRight(left);
        ViewPolicy.show(gutter);
        row.addView(gutter, left ? 0 : 1, gutterParams);
        column.setLayoutParams(KeyboardGeometry.weightedWrapParams(1f - gutterWeight));
        row.requestLayout();
    }

    /**
     * IME 窗口的导航栏跟键盘底色：Android 15 以前直接设导航栏颜色，所有版本都按底色明暗切换导航栏按钮的深浅（深色键盘配浅色按钮）。
     *
     * @param color 键盘底色（ARGB）
     * @param dark 键盘是否深色
     */
    void applyNavigationBar(int color, boolean dark) {
        if (navigationApplied && navigationColor == color && navigationDark == dark) return;
        Dialog dialog = s.getWindow();
        Window window = dialog == null ? null : dialog.getWindow();
        if (window == null) return;
        navigationApplied = true;
        navigationColor = color;
        navigationDark = dark;
        if (Build.VERSION.SDK_INT < 35) setNavigationBarColor(window, color);
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = window.getInsetsController();
            if (controller != null) {
                controller.setSystemBarsAppearance(
                    dark ? 0 : WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
                    WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
            }
        }
    }

    // Android 15 起导航栏颜色由系统按 edge-to-edge 处理，这个调用只给更早的版本。
    @SuppressWarnings("deprecation")
    private static void setNavigationBarColor(Window window, int color) {
        window.setNavigationBarColor(color);
    }
}
