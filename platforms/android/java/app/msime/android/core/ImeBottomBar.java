package app.msime.android;

import android.app.Dialog;
import android.content.res.Configuration;
import android.graphics.Color;
import android.graphics.Rect;
import android.os.Build;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewConfiguration;
import android.view.Window;
import android.view.WindowInsets;
import android.view.inputmethod.InputMethodManager;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * 键盘底栏（{@link KeyboardBottomBarPolicy}）：垫在键区下面、导航栏上面的一条，左边切换输入法，右边剪贴板，中间左右滑动移动光标。
 *
 * <p>底栏不在键盘的竖向一列里，而是叠在外框（{@link MSIMEInputService#keyboardSurface}）底部：画着的时候键盘列的底部内边距在导航栏那一截之上再加一条底栏的高度，底栏就落在这段内边距里。外框的各个覆盖面板按键盘列的底部内边距让出底边（见 `PanelSurface`），所以剪贴板、表情这些面板打开时底栏仍露在下面，点剪贴板按钮可以直接关上面板。
 */
final class ImeBottomBar {
    private final MSIMEInputService s;
    private LinearLayout bar;
    private KeyboardShortcutButton switchButton;
    private KeyboardShortcutButton clipboardButton;
    private TextView track;
    /** 中间滑动区自己的光标移动状态，与空格键的互不干扰。 */
    private final SpaceCursorMovement movement = new SpaceCursorMovement();
    /** 键盘视图收到的系统栏 inset（像素）。 */
    private final Rect insets = new Rect();
    /** 屏幕底部导航栏与系统手势区的高度（像素），见 {@link #readInsets}。 */
    private int navigationBottom;
    private int gestureBottom;
    private boolean navigationKnown;

    ImeBottomBar(MSIMEInputService s) {
        this.s = s;
    }

    /** 建好底栏并叠到外框底部；每次 onCreateInputView 新建控件后调用一次。 */
    void build(MSIMEInputService.PanelSurface surface) {
        bar = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(bar);
        bar.setContentDescription("键盘底栏");
        // 系统的输入法选择框，与 Android 自己画在导航栏里的切换按钮一致；只装了一个别的输入法时也能用，组字不结束，理由见 bindInputMethodPicker。
        switchButton = new KeyboardShortcutButton(s, KeyboardShortcutIconPolicy.Icon.GLOBE);
        ViewPolicy.setAllCapsFalse(switchButton);
        switchButton.setText("切换输入法");
        switchButton.setContentDescription("切换输入法");
        s.keyId(switchButton, "SoftGlobe");
        s.imeStyler.styleButton(switchButton, true);
        s.bindCountedAction(switchButton, this::showInputMethodPicker);
        bar.addView(switchButton, buttonParams());
        track = new TextView(s);
        ViewPolicy.setCentered(track);
        KeyboardGeometry.setKeyTextSize(track, 12);
        track.setContentDescription("左右滑动移动光标");
        bindCursorTrack(track);
        bar.addView(track, KeyboardGeometry.weightedMatchParentParams(1));
        clipboardButton = new KeyboardShortcutButton(s, KeyboardShortcutIconPolicy.Icon.CLIPBOARD);
        ViewPolicy.setAllCapsFalse(clipboardButton);
        clipboardButton.setText("剪贴板");
        clipboardButton.setContentDescription("剪贴板");
        s.imeStyler.styleButton(clipboardButton, true);
        s.bindCountedAction(clipboardButton,
            s.imeToolbar.panelToggle(() -> s.clipboardScroll, s.imePanels::showClipboardHistory));
        bar.addView(clipboardButton, buttonParams());
        ViewPolicy.hide(bar);
        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, s.pixels(KeyboardBottomBarPolicy.BAR_HEIGHT_DP),
            Gravity.BOTTOM);
        // 外框会把非 fullBleed 子视图的底边距改成键盘列的底部内边距，底栏的底边距由这里自己定。
        surface.fullBleed.add(bar);
        surface.addView(bar, params);
    }

    private LinearLayout.LayoutParams buttonParams() {
        LinearLayout.LayoutParams params = KeyboardGeometry.linearParamsPx(s.pixels(56),
            LinearLayout.LayoutParams.MATCH_PARENT);
        params.setMarginStart(s.pixels(8));
        params.setMarginEnd(s.pixels(8));
        return params;
    }

    private void showInputMethodPicker() {
        InputMethodManager manager = s.getSystemService(InputMethodManager.class);
        if (manager != null) manager.showInputMethodPicker();
    }

    /** 中间滑动区：横向越过触摸阈值后先结束组字，之后每 12 dp 移一格光标，与空格键拖动同一套步长。 */
    private void bindCursorTrack(View view) {
        final float[] originX = new float[1];
        final boolean[] dragging = new boolean[1];
        final int touchSlop = ViewConfiguration.get(s).getScaledTouchSlop();
        view.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    originX[0] = event.getX();
                    dragging[0] = false;
                    movement.cancel();
                    view.getParent().requestDisallowInterceptTouchEvent(true);
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (!dragging[0]) {
                        if (Math.abs(event.getX() - originX[0]) <= touchSlop || s.connection == null) return true;
                        s.command(2);
                        movement.begin(originX[0], s.connection);
                        dragging[0] = movement.isActive();
                        if (dragging[0]) track.setText("移动光标");
                    }
                    if (dragging[0]) {
                        s.imeBottomRow.moveEditorCursor(movement.advance(event.getX(), s.connection, s.pixels(12)));
                        if (!movement.isActive()) endDrag(dragging);
                    }
                }
                case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    view.getParent().requestDisallowInterceptTouchEvent(false);
                    endDrag(dragging);
                }
                default -> { }
            }
            return true;
        });
    }

    private void endDrag(boolean[] dragging) {
        dragging[0] = false;
        movement.cancel();
        track.setText("");
    }

    /**
     * 键盘视图收到新的 insets 时调用：记下视图的系统栏 inset，再读屏幕底部导航栏与系统手势区的高度。
     *
     * <p>导航栏高度从输入法窗口的 WindowMetrics 取，与键盘视图收到的 inset 取较大的那个。Android 14 及以前输入法窗口默认停在导航栏上方，视图收到的底部 inset 是 0，只看它会把三键导航当成没有导航栏；WindowMetrics 按整块屏幕的范围算，导航栏总在里面。Android 11 的 WindowMetrics 在有父窗口时按窗口自己的范围算 inset，可能同样是 0，所以 Android 11 及以前不画底栏。
     */
    void readInsets(WindowInsets viewInsets) {
        insets.set(WindowLayout.systemBars(viewInsets));
        navigationKnown = false;
        if (Build.VERSION.SDK_INT < 31) {
            navigationBottom = insets.bottom;
            gestureBottom = insets.bottom;
            return;
        }
        int navigation = viewInsets.getInsets(WindowInsets.Type.navigationBars()).bottom;
        int gesture = viewInsets.getInsets(WindowInsets.Type.mandatorySystemGestures()).bottom;
        Dialog dialog = s.getWindow();
        Window window = dialog == null ? null : dialog.getWindow();
        if (window != null) {
            WindowInsets screen = window.getWindowManager().getCurrentWindowMetrics().getWindowInsets();
            navigation = Math.max(navigation, screen.getInsets(WindowInsets.Type.navigationBars()).bottom);
            gesture = Math.max(gesture, screen.getInsets(WindowInsets.Type.mandatorySystemGestures()).bottom);
            // 只有视图的 inset 时同样分不出三键导航，窗口还没建好就先不画。
            navigationKnown = true;
        }
        navigationBottom = navigation;
        gestureBottom = gesture;
    }

    /** 键盘视图收到的系统栏 inset（像素）。 */
    Rect insets() {
        return insets;
    }

    /** 底栏此刻画不画（{@link KeyboardBottomBarPolicy#shown}）。 */
    boolean shown() {
        if (bar == null) return false;
        Configuration configuration = s.getResources().getConfiguration();
        boolean phoneLandscape = configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
            && !KeyboardFormFactorPolicy.expanded(configuration.smallestScreenWidthDp);
        return KeyboardBottomBarPolicy.shown(s.localSettings.bool(AndroidLocalSettings.BOTTOM_BAR),
            systemDrawsImeButtons(), navigationKnown, s.floatingDrawn(), s.hardwareKeysCollapsed(), phoneLandscape,
            KeyboardGeometry.fromPixels(s, navigationBottom));
    }

    /**
     * 系统是否自己在输入法窗口底部画了收起和切换按钮。Android 13 起原生系统在手势导航下把这一条（`android.inputmethodservice.navigationbar.NavigationBarFrame`）直接加在输入法窗口的 decor 下，三键导航或系统不画时它不存在或不可见。没有公开接口能问，只能按类名看 decor 的直接子视图；国产系统不画这条时，这里是假。
     */
    private boolean systemDrawsImeButtons() {
        Dialog dialog = s.getWindow();
        Window window = dialog == null ? null : dialog.getWindow();
        if (window == null || !(window.peekDecorView() instanceof ViewGroup decor)) return false;
        for (int index = 0; index < decor.getChildCount(); index++) {
            View child = decor.getChildAt(index);
            if (child.getVisibility() == View.VISIBLE
                    && "NavigationBarFrame".equals(child.getClass().getSimpleName())) return true;
        }
        return false;
    }

    /**
     * 按当前状态给键盘列套底部内边距并摆放底栏：浮动时四边都不留；停靠时留出系统栏，底栏画着时底部再加上底栏和它下面让出的手势区。insets 变化和每次 render 都调用，状态没变时不触发重新布局。
     *
     * @param keyboard 键盘的竖向一列
     */
    void apply(LinearLayout keyboard) {
        if (keyboard == null) return;
        if (s.floatingDrawn()) {
            if (bar != null) ViewPolicy.hide(bar);
            setPadding(keyboard, 0, 0, 0, 0);
            return;
        }
        boolean shown = shown();
        int bottom = insets.bottom;
        if (shown) {
            int margin = KeyboardBottomBarPolicy.barBottomPx(insets.bottom, navigationBottom, gestureBottom);
            if (bar.getLayoutParams() instanceof FrameLayout.LayoutParams params && params.bottomMargin != margin) {
                params.bottomMargin = margin;
                bar.setLayoutParams(params);
            }
            bottom = margin + s.pixels(KeyboardBottomBarPolicy.BAR_HEIGHT_DP);
        }
        if (bar != null && (bar.getVisibility() == View.VISIBLE) != shown) ViewPolicy.setVisible(bar, shown);
        setPadding(keyboard, insets.left, insets.top, insets.right, bottom);
    }

    private static void setPadding(View view, int left, int top, int right, int bottom) {
        if (view.getPaddingLeft() == left && view.getPaddingTop() == top
                && view.getPaddingRight() == right && view.getPaddingBottom() == bottom) return;
        ViewPolicy.setPadding(view, left, top, right, bottom);
    }

    /** 每次 render 末尾按皮肤给两个按钮上色；剪贴板面板开着时剪贴板按钮画成选中。 */
    void style(int iconColor, int activeIconColor, int activeBackgroundColor) {
        if (bar == null) return;
        for (KeyboardShortcutButton button : new KeyboardShortcutButton[] {switchButton, clipboardButton}) {
            button.setActiveFill(activeBackgroundColor);
            button.setIconColors(iconColor, activeIconColor);
        }
        ViewPolicy.setSelected(clipboardButton,
            s.clipboardScroll != null && s.clipboardScroll.getVisibility() == View.VISIBLE);
        ViewPolicy.setTextColor(track, Color.argb(160, Color.red(iconColor), Color.green(iconColor),
            Color.blue(iconColor)));
    }
}
