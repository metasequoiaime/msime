package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.app.Dialog;
import android.content.Context;
import android.graphics.Color;
import android.content.res.Configuration;
import android.os.Build;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsetsController;
import android.widget.LinearLayout;

/**
 * 包住键区的容器：键行与底行经这里放进键盘的竖向布局，导航栏颜色在这里跟随键盘底色。
 *
 * <p>单手模式（`touch_one_handed` 为 `left` / `right`）时，键行与底行收窄到总宽的 85% 推向那一侧（大屏上「侧栏 + 键区」缩到手机大小贴到屏幕那一侧，见 {@link KeyboardFormFactorPolicy#oneHandedBlockWidthDp}），另一侧是 {@link OneHandGutterView}：‹ 换到另一侧、⤢ 退出单手模式，两者都只写偏好（经 SVC 的 toggleOneHanded）。工具栏与候选条不在这里，保持全宽。`off` 时侧栏不显示，键区占满全宽，与原来相同。
 *
 * <p>大屏单手时空出来的那一侧可以放侧边数字键盘（#10203，{@link ImeSidePad}）：贴在屏幕边上，和侧栏之间是一块不接触摸的空白。它和侧栏、键区在同一行里，外接键盘收起整行、半透明候选画布盖住整行时一起收起或隐藏。
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
    /** 大屏单手时空出来那一侧的数字键盘和它旁边的空白。 */
    final ImeSidePad sidePad;
    private String appliedMode = "";
    private boolean keysCollapsed;
    private boolean keysCovered;

    ImeFrame(MSIMEInputService s) {
        this.s = s;
        sidePad = new ImeSidePad(s);
    }

    /** 侧栏在哪一边：键盘靠右时在左（索引 0），靠左时在右。 */
    static boolean gutterOnLeft(String mode) {
        return "right".equals(mode);
    }

    /** 偏好值是否开启单手模式。 */
    static boolean oneHanded(String mode) {
        return "left".equals(mode) || "right".equals(mode);
    }

    /** 实际生效的单手取值：分离式键盘与浮动键盘画着的时候单手模式让位，返回 `off`；存着的偏好不变。 */
    static String effectiveOneHanded(MSIMEInputService s) {
        String stored = SplitKeyboardPolicy.effectiveOneHanded(s.oneHandedMode,
            s.splitKeyboardDrawn() || s.floatingDrawn());
        return oneHanded(stored) ? stored : "off";
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
                    if (!ViewPolicy.isGone(keys.getChildAt(index))) {
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
        column = LayoutPolicy.column(s);
        sidePad.forget();
        row.keys = column;
        gutter = new OneHandGutterView(s);
        gutter.setOnSwap(() -> s.toggleOneHanded(true));
        gutter.setOnExit(() -> {
            if (oneHanded(s.oneHandedMode)) s.toggleOneHanded(false);
        });
        ViewPolicy.hide(gutter);
        row.addView(column, LayoutPolicy.weightedWrapParams(1));
        keyboard.addView(row, LayoutPolicy.matchWidthWrapParams());
        appliedMode = "";
        applyKeysVisibility();
        return column;
    }

    /** 外接键盘的候选条模式里收起整行键区（{@link HardwareKeyboardModePolicy#keysCollapsed}）；单手侧栏在同一行里，一起收起。 */
    void setKeysCollapsed(boolean collapsed) {
        keysCollapsed = collapsed;
        applyKeysVisibility();
    }

    /** 半透明候选画布盖住键区时保留测量高度，同时隐藏按键、触摸和可访问性入口。 */
    void setKeysCovered(boolean covered) {
        keysCovered = covered;
        applyKeysVisibility();
    }

    private void applyKeysVisibility() {
        if (row == null) return;
        int visibility = keysCollapsed ? View.GONE : keysCovered ? View.INVISIBLE : View.VISIBLE;
        if (row.getVisibility() != visibility) row.setVisibility(visibility);
    }

    /** 按默认布局参数放入键区。 */
    void wrap(ViewGroup keyArea) {
        column().addView(keyArea, LayoutPolicy.matchWidthWrapParams());
        applyOneHanded();
    }

    /** 按给定布局参数放入键区。 */
    void wrap(ViewGroup keyArea, ViewGroup.LayoutParams params) {
        column().addView(keyArea, params);
        applyOneHanded();
    }

    /** 按当前 `touch_one_handed` 摆放侧栏与键区，大屏上再按 `touch_one_handed_side_panel` 和此刻的键面摆上或摘下侧边数字键盘；与上次相同时只刷新颜色。渲染与换肤时调用，切 123、切中英、换方案都经过渲染。分离式键盘画着的时候单手模式不生效（存着的值不变，回到不分离时自动恢复），见 {@link SplitKeyboardPolicy#effectiveOneHanded}。 */
    void applyOneHanded() {
        if (row == null || gutter == null || column == null) return;
        // 浮动键盘本身就是一块窄面板，单手模式与分离式键盘一样不生效，存着的值不变。
        String mode = effectiveOneHanded(s);
        if (s.skin != null) {
            gutter.setColors(Color.parseColor(s.skin.keyBackground()), Color.parseColor(s.skin.toolbarIcon()));
        }
        Configuration configuration = s.getResources().getConfiguration();
        int blockDp = oneHanded(mode) ? KeyboardFormFactorPolicy.oneHandedBlockWidthDp(
            configuration.smallestScreenWidthDp, configuration.screenWidthDp) : 0;
        int padDp = blockDp > 0 ? KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(
            configuration.smallestScreenWidthDp, configuration.screenWidthDp) : 0;
        // 侧边数字键盘只在九键一族的字母层画；键面按键行建出来时的布局算，不按还没重建的会话快照。
        boolean padShown = padDp > 0 && OneHandSidePadPolicy.shown(s.oneHandedSidePanel, mode, padDp,
            s.imeLetterRows.drawnTouchLayout(), s.keyboardLayer, s.imeLetterRows.numberPadDrawn());
        String applied = mode + ":" + blockDp + ":" + (padShown ? padDp : 0);
        if (applied.equals(appliedMode)) return;
        boolean surfaceChanged = !appliedMode.startsWith(mode + ":");
        appliedMode = applied;
        // 大屏上单手模式开关会改变键盘表面铺不铺满窗口（KeyboardFormFactorPolicy.surfaceWidthDp），表面要跟着重排。
        if (surfaceChanged) s.imeStyler.applyKeyboardSurfaceGeometry();
        if (gutter.getParent() != null) row.removeView(gutter);
        sidePad.detachFrom(row);
        if (!oneHanded(mode)) {
            ViewPolicy.hide(gutter);
            row.setGravity(Gravity.NO_GRAVITY);
            column.setLayoutParams(LayoutPolicy.weightedWrapParams(1));
            row.requestLayout();
            return;
        }
        boolean left = gutterOnLeft(mode);
        LinearLayout.LayoutParams gutterParams;
        if (blockDp > 0) {
            // 大屏：侧栏与键区按手机大小取固定宽度，整块贴到键盘所在的那一侧，另一侧空着。
            int block = s.pixels(blockDp);
            gutterParams = LayoutPolicy.linearParamsPx(OneHandGutterView.gutterWidth(block),
                LinearLayout.LayoutParams.MATCH_PARENT);
            column.setLayoutParams(LayoutPolicy.linearParamsPx(OneHandGutterView.keysWidth(block),
                LinearLayout.LayoutParams.WRAP_CONTENT));
            row.setGravity(left ? Gravity.END : Gravity.START);
        } else {
            float gutterWeight = OneHandGutterView.GUTTER_FRACTION;
            gutterParams = LayoutPolicy.weightedMatchParentParams(gutterWeight);
            column.setLayoutParams(LayoutPolicy.weightedWrapParams(1f - gutterWeight));
            row.setGravity(Gravity.NO_GRAVITY);
        }
        gutter.setKeyboardOnRight(left);
        ViewPolicy.show(gutter);
        row.addView(gutter, left ? 0 : 1, gutterParams);
        if (padShown) {
            // 侧边数字键盘贴在空出来那一侧的屏幕边上：键在右时排成 [面板, 空白, 侧栏, 键列]，键在左时 [键列, 侧栏, 空白, 面板]。空白吃掉剩下的宽度，行不再靠重力把整块推到一侧。面板高度随行（就是键列的高度），里面的键块贴底与九键网格和底行逐行对齐。
            row.setGravity(Gravity.NO_GRAVITY);
            LinearLayout.LayoutParams padParams = LayoutPolicy.linearParamsPx(s.pixels(padDp),
                LinearLayout.LayoutParams.MATCH_PARENT);
            if (left) {
                row.addView(sidePad.spacer(), 0, LayoutPolicy.weightedMatchParentParams(1));
                row.addView(sidePad.pad(), 0, padParams);
            } else {
                row.addView(sidePad.spacer(), LayoutPolicy.weightedMatchParentParams(1));
                row.addView(sidePad.pad(), padParams);
            }
            // 摘下期间换过皮肤时整树上色走不到它，摆上时按当前皮肤补一遍。
            s.imeStyler.applySkinToView(sidePad.pad());
        }
        row.requestLayout();
    }

    /**
     * IME 窗口的导航栏跟键盘底色：Android 15 以前直接设导航栏颜色，所有版本都按底色明暗切换导航栏按钮的深浅（深色键盘配浅色按钮）。
     *
     * <p>Android 10 起系统默认对三键导航栏做对比度增强：导航栏透明时在下面铺一层接近不透明的白（深色模式是黑）衬底。Android 15 的 edge-to-edge 让导航栏一律透明，这层衬底就盖住了键盘底色，键盘下面多出一条白边。按钮深浅已经按底色切换，所以关掉它，让键盘底色一直铺到屏幕底边。
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
        if (Build.VERSION.SDK_INT >= 29) window.setNavigationBarContrastEnforced(false);
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
