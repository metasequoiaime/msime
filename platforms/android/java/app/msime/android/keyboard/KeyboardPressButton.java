package app.msime.android;

import android.annotation.SuppressLint;
import android.graphics.drawable.Drawable;
import android.view.MotionEvent;
import android.widget.Button;

/** Matches Apple's key press feedback without changing the button's layout or input timing. */
public class KeyboardPressButton extends Button {
    private KeyboardKeyRole role;
    private boolean dispatchingTouch;
    private boolean staleRelease;
    private KeyboardSkin faceSkin;
    private KeyboardKeyRole faceRole;
    private boolean faceSelected;
    private float faceDensity;
    private Drawable faceDrawable;

    public KeyboardPressButton(android.content.Context context) {
        super(context);
    }

    /**
     * The face the shared style pass should give this button.
     *
     * <p>It is carried on the view rather than decided at style time because that pass re-walks the
     * whole tree on every render and has no other way to tell a toolbar glyph from a key cap. Left
     * unset, the button keeps following the accessibility description the pass has always read, so
     * only the controls that asked for a role change appearance.
     */
    public void setKeyboardRole(KeyboardKeyRole value) { role = value; }

    /** The requested role, or {@code null} to follow the shared derivation. */
    public KeyboardKeyRole keyboardRole() { return role; }

    /**
     * 样式通道上一次给这个键装的键帽是否原样还在：同一个皮肤对象（`KeyboardSkin` 不可变）、同一个角色、同样的选中状态和屏幕密度，背景也没有被别处换掉。
     *
     * <p>样式通道每次 render 都会走一遍整棵树。键帽由这几项完全决定，它们都没变时再 new 一个 Drawable 换上去，画出来一模一样，却会让这个键的显示列表失效，于是每按一个键整块键盘都要重录一遍。
     */
    public boolean keepsFace(KeyboardSkin skin, KeyboardKeyRole role, boolean selected, float density) {
        return faceDrawable != null && getBackground() == faceDrawable && faceSkin == skin
            && faceRole == role && faceSelected == selected && faceDensity == density;
    }

    /** 记下样式通道刚装上的键帽和决定它的几项，供 {@link #keepsFace} 比较。 */
    public void rememberFace(KeyboardSkin skin, KeyboardKeyRole role, boolean selected, float density) {
        faceSkin = skin;
        faceRole = role;
        faceSelected = selected;
        faceDensity = density;
        faceDrawable = getBackground();
    }

    /**
     * 同一个键连按时，不让上一次按下留下的松开把这一次按下吞掉。
     *
     * <p>AOSP `View` 在 `ACTION_UP` 时先 `post(mPerformClick)` 再 `post(mUnsetPressedState)`。点击处理（引擎调用加整块键盘重绘）跑得久的时候，同一个键的下一个 `ACTION_DOWN` 会排在那条 `UnsetPressedState` 之前送到：DOWN 发现键还按着，什么也不改；随后迟到的 `setPressed(false)` 把这次新按下清掉，于是这次的 `ACTION_UP` 看到既不是 PRESSED 也不是 PREPRESSED，框架不发点击，这一下按键就丢了（真机上九键连按 4、4 能复现）。
     *
     * <p>这里只认这一种情况：DOWN 到达时键仍处于上一次手势留下的按下状态，就把这次手势里第一个不是来自触摸分发的 `setPressed(false)` 当作那条迟到的松开忽略掉。滑出、`ACTION_CANCEL`、`ACTION_UP` 都发生在触摸分发之内，照常生效；手势结束（UP 或 CANCEL）或离开窗口时标记一并清掉，所以按下状态和按下动画都不会卡住。点击仍然只由框架自己在 UP 时发出，长按（九键的长按选项）、滑出取消和无障碍的 `performClick` 都走原来的路径，不会多出一次点击。
     */
    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        boolean outer = dispatchingTouch;
        dispatchingTouch = true;
        boolean handled = super.dispatchTouchEvent(event);
        dispatchingTouch = outer;
        return handled;
    }

    // Lint 要求重写 `onTouchEvent` 的地方自己调 `performClick`；这里只记一个标记，点击仍由 `super.onTouchEvent` 决定和发出，自己再调一次就成了双击。
    @SuppressLint("ClickableViewAccessibility")
    @Override public boolean onTouchEvent(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> staleRelease = isPressed() && isEnabled();
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> staleRelease = false;
            default -> { }
        }
        return super.onTouchEvent(event);
    }

    @Override public void setPressed(boolean pressed) {
        if (!pressed && staleRelease && !dispatchingTouch) {
            staleRelease = false;
            return;
        }
        boolean changed = pressed != isPressed();
        super.setPressed(pressed);
        if (changed) updatePressFeedback();
    }

    @Override public void setEnabled(boolean enabled) {
        super.setEnabled(enabled);
        if (!enabled) updatePressFeedback();
    }

    @Override protected void onDetachedFromWindow() {
        // 离开窗口时框架会在这之后用 `setPressed(false)` 撤掉还没执行的松开，那一次必须生效。
        staleRelease = false;
        animate().cancel();
        KeyboardPressFeedback.reset(this);
        super.onDetachedFromWindow();
    }

    private void updatePressFeedback() {
        KeyboardPressFeedback.update(this, isPressed());
    }
}
