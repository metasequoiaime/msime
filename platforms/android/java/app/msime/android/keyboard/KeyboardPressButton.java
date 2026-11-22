package app.msime.android;

import android.annotation.SuppressLint;
import android.graphics.Canvas;
import android.graphics.drawable.Drawable;
import android.view.MotionEvent;
import android.widget.Button;
import java.util.Set;

/**
 * Matches Apple's key press feedback without changing the button's layout or input timing.
 *
 * <p>放进键组（{@link #setFaceGroup}）的键由 {@link KeyFaceRenderer} 画键面：主标签和提示按键组算好的字号和位置画，TextView 的文字只作节点文字（读屏、统计、按键气泡），不再排版、不再自己画，尺寸照旧由父视图给。没放进键组的键照原来的方式画。
 */
public class KeyboardPressButton extends Button implements KeyFaceGroup.Member {
    private KeyboardKeyRole role;
    private boolean dispatchingTouch;
    private boolean staleRelease;
    private Runnable pressReleaseListener;
    private boolean animatesPressFeedback = true;
    private KeyboardSkin faceSkin;
    private KeyboardKeyRole faceRole;
    private boolean faceSelected;
    private float faceDensity;
    private Drawable faceDrawable;
    /** 放进键组后才有；TextView 的构造函数里会先调 {@link #onTextChanged}，那时它还是 null。 */
    private KeyFaceRenderer keyFace;
    /** 自己和祖先都可见（{@link #onVisibilityAggregated}）；隐藏的键不算进键组的最小键格。 */
    private boolean visibleForFace = true;
    /** 键组里的键正在换节点文字：这期间 TextView 请求的重新布局只换成重画（{@link #requestLayout}）。 */
    private boolean settingFaceText;

    public KeyboardPressButton(android.content.Context context) {
        super(context);
    }

    /**
     * 把这个键放进键组（{@code null} 拿出来，回到 TextView 自己排版和绘制）。{@code slots} 是它可能用到的提示位置。
     *
     * <p>放进键组时关掉 TextView 的自动字号：键面由键组定字号，自动字号只会在每次换文字时白白试排一遍（离开键组的 ⇧、⌫ 本来就不用自动字号，不用恢复）。
     */
    public void setFaceGroup(KeyFaceGroup group, Set<KeyFaceSlot> slots) {
        if (keyFace != null) keyFace.group().detach(this);
        keyFace = group == null ? null : new KeyFaceRenderer(group);
        if (group != null) {
            if (getAutoSizeTextType() != AUTO_SIZE_TEXT_TYPE_NONE) setAutoSizeTextTypeWithDefaults(AUTO_SIZE_TEXT_TYPE_NONE);
            group.attach(this, slots);
            reportCell();
        }
        requestLayout();
        invalidate();
    }

    /** 所在的键组；没放进键组时为 {@code null}。 */
    public KeyFaceGroup faceGroup() { return keyFace == null ? null : keyFace.group(); }

    /** 键组此刻给这个键的参数，设备验收读它；没放进键组或还没布局好时为 {@code null}。 */
    public KeyFaceParams faceParams() { return keyFace == null ? null : keyFace.group().params(); }

    @Override public String faceText(KeyFaceSlot slot) {
        return slot == KeyFaceSlot.MAIN ? String.valueOf(getText()) : "";
    }

    /** 这个位置用的颜色；主标签用皮肤给的文字色，子类给提示的颜色。 */
    protected int faceColor(KeyFaceSlot slot) { return getCurrentTextColor(); }

    @Override public void faceParamsChanged() {
        if (keyFace == null) return;
        keyFace.stale();
        invalidate();
    }

    /** 画法变了而文字没变：放进键组时下次绘制前重新定位；都要重画。 */
    protected final void faceRedraw() {
        if (keyFace != null) keyFace.stale();
        invalidate();
    }

    /** 提示的文字变了：内容域里没有就补进键组，下次绘制前重新定位。 */
    protected final void faceTextChanged(KeyFaceSlot slot, String text) {
        if (keyFace == null) return;
        keyFace.group().include(slot, text);
        keyFace.stale();
        invalidate();
    }

    /**
     * 键组里的键换文字不重新布局：键面不按文字量尺寸，尺寸只由父视图给。TextView 留着一份自己的文字排版（绘制前的 `onPreDraw` 会补建），换文字时按它重排，文字高度一变就请求整个键盘重新布局；这里把这一次请求换成重画。
     */
    @Override public void setText(CharSequence text, BufferType type) {
        if (keyFace == null) {
            super.setText(text, type);
            return;
        }
        settingFaceText = true;
        try {
            super.setText(text, type);
        } finally {
            settingFaceText = false;
        }
    }

    @Override public void requestLayout() {
        if (settingFaceText) {
            invalidate();
            return;
        }
        super.requestLayout();
    }

    @Override protected void onTextChanged(CharSequence text, int start, int lengthBefore, int lengthAfter) {
        super.onTextChanged(text, start, lengthBefore, lengthAfter);
        // 子类的字段在 TextView 的构造函数里还没初始化，那时也还没放进键组。
        if (keyFace != null) faceTextChanged(KeyFaceSlot.MAIN, faceText(KeyFaceSlot.MAIN));
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        // 键组里的键不按文字量尺寸：父视图给的就是键格，量文字只会白排一遍版。
        if (keyFace != null && MeasureSpec.getMode(widthMeasureSpec) == MeasureSpec.EXACTLY
                && MeasureSpec.getMode(heightMeasureSpec) == MeasureSpec.EXACTLY) {
            setMeasuredDimension(MeasureSpec.getSize(widthMeasureSpec), MeasureSpec.getSize(heightMeasureSpec));
            return;
        }
        super.onMeasure(widthMeasureSpec, heightMeasureSpec);
    }

    /** 把此刻的键格报给键组；隐藏时报 0，不算进最小键格。 */
    private void reportCell() {
        if (keyFace == null) return;
        keyFace.group().cellChanged(this, visibleForFace ? getWidth() : 0, visibleForFace ? getHeight() : 0);
    }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        super.onSizeChanged(width, height, oldWidth, oldHeight);
        reportCell();
    }

    @Override public void onVisibilityAggregated(boolean isVisible) {
        super.onVisibilityAggregated(isVisible);
        visibleForFace = isVisible;
        reportCell();
    }

    @Override protected void onAttachedToWindow() {
        super.onAttachedToWindow();
        reportCell();
    }

    @Override protected void onDraw(Canvas canvas) {
        if (keyFace == null) {
            super.onDraw(canvas);
            return;
        }
        int saved = beginFaceDraw(canvas);
        keyFace.draw(canvas, this);
        canvas.restoreToCount(saved);
    }

    /**
     * 按键格坐标画键面：抵消 TextView 自己设的滚动。键组里的键不走 TextView 的测量，TextView 留着的文字排版可能还是按旧宽度排的；它在每次绘制前（`onPreDraw`）仍按那份排版把内容滚到「放得下」的位置，画布随之平移，键面就整体偏到一边（九键右列的「重输」在模拟器上偏右约 26 px）。
     *
     * @return 交给 {@link Canvas#restoreToCount} 的值
     */
    protected final int beginFaceDraw(Canvas canvas) {
        int saved = canvas.save();
        canvas.translate(getScrollX(), getScrollY());
        return saved;
    }

    /** 候选词块可关闭过渡，按压样式与输入时机保持不变。 */
    public void setAnimatesPressFeedback(boolean value) {
        animatesPressFeedback = value;
        updatePressFeedback();
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

    /** 按下状态真正清除后通知调用方；迟到且被忽略的松开不通知。 */
    public void setPressReleaseListener(Runnable listener) {
        pressReleaseListener = listener;
    }

    @Override public void setPressed(boolean pressed) {
        if (!pressed && staleRelease && !dispatchingTouch) {
            staleRelease = false;
            return;
        }
        boolean changed = pressed != isPressed();
        super.setPressed(pressed);
        if (changed) {
            updatePressFeedback();
            if (!pressed && pressReleaseListener != null) pressReleaseListener.run();
        }
    }

    @Override public void setEnabled(boolean enabled) {
        super.setEnabled(enabled);
        if (!enabled) updatePressFeedback();
    }

    @Override protected void onDetachedFromWindow() {
        // 离开窗口时框架会在这之后用 `setPressed(false)` 撤掉还没执行的松开，那一次必须生效。
        staleRelease = false;
        KeyPressAnimator.cancel(this);
        animate().cancel();
        KeyboardPressFeedback.reset(this);
        if (keyFace != null) keyFace.group().cellChanged(this, 0, 0);
        super.onDetachedFromWindow();
    }

    private void updatePressFeedback() {
        KeyboardPressFeedback.update(this, isPressed(), animatesPressFeedback);
    }
}
