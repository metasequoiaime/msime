package app.msime.android;

import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.os.Bundle;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.Switch;
import android.widget.TextView;

/**
 * Transparent Android equivalent of Apple's live keyboard-layout adjustment surface.
 * The keyboard remains visible underneath; the body consumes drag gestures without
 * forwarding them to keys, while the top bar exposes reset, voice-shortcut, close and
 * height controls.
 */
public final class KeyboardLayoutAdjustView extends FrameLayout {
    public interface Listener {
        void keySpacing(int tenths);
        void rowSpacing(int tenths);
        void height(int adjustment);
        void voiceShortcut(boolean enabled);
        void commit();
        void reset();
        void close();
    }

    /** Long enough for a run of accessibility steps to settle, short enough to feel immediate. */
    private static final long COMMIT_DELAY_MILLIS = 250;
    private static final int BAR_HEIGHT_DP = 52;
    private static final int SPACING_MARGIN_DP = 8;
    private final Listener listener;
    private final LinearLayout bar;
    private final TextView hint;
    private final Switch voiceShortcut;
    private final Button resetButton;
    private int keySpacing;
    private int rowSpacing;
    private int heightAdjustment;
    private boolean trackingHeight;
    private boolean adjustmentsEnabled = true;
    private final Runnable commitAdjustment = this::commitAdjustment;
    private float downX;
    private float downY;
    private int baseKeySpacing;
    private int baseRowSpacing;
    private int baseHeight;
    private KeyboardLayoutAdjustPolicy.Axis axis;

    public KeyboardLayoutAdjustView(android.content.Context context, Listener listener) {
        super(context);
        this.listener = listener;
        setClickable(true);
        setFocusable(true);
        setContentDescription("键盘布局调整；键盘上左右拖动调整按键间距，上下拖动调整行间距");
        setBackgroundColor(Color.TRANSPARENT);

        bar = new LinearLayout(context);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setGravity(android.view.Gravity.CENTER_VERTICAL);
        bar.setPadding(KeyboardGeometry.pixels(getContext(), 8), KeyboardGeometry.pixels(getContext(), 4),
            KeyboardGeometry.pixels(getContext(), 8), KeyboardGeometry.pixels(getContext(), 4));
        bar.setContentDescription("键盘高度调整工具栏");
        bar.setFocusable(true);
        bar.setOnTouchListener((ignored, event) -> handleHeightGesture(event));
        addView(bar, barParams());

        resetButton = button("恢复默认", "恢复键盘布局默认值", listener::reset);
        bar.addView(resetButton, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));

        hint = new TextView(context);
        hint.setGravity(android.view.Gravity.CENTER);
        hint.setTextSize(13);
        hint.setMaxLines(2);
        hint.setContentDescription("布局调整说明");
        bar.addView(hint, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 1));

        voiceShortcut = new Switch(context);
        voiceShortcut.setText("语音");
        voiceShortcut.setContentDescription("顶部语音入口");
        voiceShortcut.setOnCheckedChangeListener((ignored, checked) -> listener.voiceShortcut(checked));
        bar.addView(voiceShortcut, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));

        Button close = button("完成", "返回键盘", listener::close);
        bar.addView(close, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        update(KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS,
            KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS,
            KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_DP, false);
    }

    public void update(int keySpacing, int rowSpacing, int heightAdjustment,
            boolean voiceShortcutEnabled) {
        this.keySpacing = KeyboardGeometry.keySpacing(keySpacing);
        this.rowSpacing = KeyboardGeometry.rowSpacing(rowSpacing);
        this.heightAdjustment = KeyboardGeometry.heightAdjustment(heightAdjustment);
        if (voiceShortcut.isChecked() != voiceShortcutEnabled)
            voiceShortcut.setChecked(voiceShortcutEnabled);
        updateHint(null);
    }

    public void updateSkin(KeyboardSkin skin) {
        int keyBackground = color(skin.keyBackground());
        int foreground = color(skin.keyForeground());
        int accent = color(skin.accent());
        GradientDrawable surface = new GradientDrawable();
        surface.setColor(keyBackground);
        surface.setCornerRadius(KeyboardGeometry.pixels(getContext(), 10));
        surface.setStroke(Math.max(1, KeyboardGeometry.pixels(getContext(), 1)), accent);
        bar.setBackground(surface);
        hint.setTextColor(foreground);
        voiceShortcut.setTextColor(foreground);
        for (int index = 0; index < bar.getChildCount(); index++) {
            View child = bar.getChildAt(index);
            if (child instanceof Button) {
                Button button = (Button) child;
                // These sit on the keyboard's action-key fill, which is where the host's own skin
                // pass puts them, so they take the colour that fill is paired with. `accent` is
                // that same fill in the shipped skins: 恢复默认 and 完成 were dark green text on a
                // dark green button, and the bar read as three blank tiles.
                button.setTextColor(color(skin.functionForeground()));
                button.setAllCaps(false);
            }
        }
    }

    public void setAdjustmentsEnabled(boolean enabled) {
        adjustmentsEnabled = enabled;
        resetButton.setEnabled(enabled);
        voiceShortcut.setEnabled(enabled);
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        return handleSpacingGesture(event);
    }

    private boolean handleSpacingGesture(MotionEvent event) {
        if (!adjustmentsEnabled) return false;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                begin(event, false);
                return true;
            }
            case MotionEvent.ACTION_MOVE -> {
                if (trackingHeight) return true;
                float translationX = dpFromPixels(event.getX() - downX);
                float translationY = dpFromPixels(event.getY() - downY);
                axis = KeyboardLayoutAdjustPolicy.chooseAxis(translationX, translationY, axis);
                if (axis == KeyboardLayoutAdjustPolicy.Axis.HORIZONTAL) {
                    keySpacing = KeyboardLayoutAdjustPolicy.keySpacingFromDrag(
                        baseKeySpacing, translationX);
                    listener.keySpacing(keySpacing);
                    updateHint("按键间距 " + KeyboardGeometry.display(keySpacing));
                } else if (axis == KeyboardLayoutAdjustPolicy.Axis.VERTICAL) {
                    rowSpacing = KeyboardLayoutAdjustPolicy.rowSpacingFromDrag(
                        baseRowSpacing, translationY);
                    listener.rowSpacing(rowSpacing);
                    updateHint("行间距 " + KeyboardGeometry.display(rowSpacing));
                }
                return true;
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                axis = null;
                updateHint(null);
                listener.commit();
                return true;
            }
            default -> { return true; }
        }
    }

    private boolean handleHeightGesture(MotionEvent event) {
        if (!adjustmentsEnabled) return false;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                begin(event, true);
                return true;
            }
            case MotionEvent.ACTION_MOVE -> {
                float translationY = dpFromPixels(event.getY() - downY);
                heightAdjustment = KeyboardLayoutAdjustPolicy.heightFromDrag(baseHeight, translationY);
                listener.height(heightAdjustment);
                updateHint("键盘高度 " + KeyboardGeometry.displayHeight(heightAdjustment));
                return true;
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                trackingHeight = false;
                updateHint(null);
                listener.commit();
                return true;
            }
            default -> { return true; }
        }
    }

    private void begin(MotionEvent event, boolean height) {
        downX = event.getX();
        downY = event.getY();
        baseKeySpacing = keySpacing;
        baseRowSpacing = rowSpacing;
        baseHeight = heightAdjustment;
        axis = null;
        trackingHeight = height;
    }

    private Button button(String title, String description, Runnable action) {
        Button button = new KeyboardPressButton(getContext());
        button.setAllCaps(false);
        button.setText(title);
        button.setContentDescription(description);
        button.setOnClickListener(ignored -> action.run());
        return button;
    }

    private FrameLayout.LayoutParams barParams() {
        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
            LayoutParams.MATCH_PARENT, KeyboardGeometry.pixels(getContext(), BAR_HEIGHT_DP));
        params.setMargins(KeyboardGeometry.pixels(getContext(), SPACING_MARGIN_DP),
            KeyboardGeometry.pixels(getContext(), 4), KeyboardGeometry.pixels(getContext(), SPACING_MARGIN_DP), 0);
        return params;
    }

    private void updateHint(String override) {
        hint.setText(override == null
            ? "拖把手改高度；键盘左右拖改键距、上下拖改行距"
            : override);
        hint.setContentDescription(override == null ? "布局调整说明" : override);
    }

    private float dpFromPixels(float pixels) {
        return KeyboardGeometry.fromPixels(getContext(), pixels);
    }

    private static int color(String value) {
        return ColorPolicy.parse(value, Color.WHITE);
    }

    /**
     * A drag previews while it moves and saves when the finger lifts. An accessibility adjustment
     * has no lift, so each step has to save on its own; previewing alone left the new height to be
     * overwritten by the next preferences apply, and the value bounced straight back.
     */
    @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD) {
            return adjustHeight(heightAdjustment + 2);
        }
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD) {
            return adjustHeight(heightAdjustment - 2);
        }
        return super.performAccessibilityAction(action, arguments);
    }

    private boolean adjustHeight(int value) {
        heightAdjustment = KeyboardGeometry.heightAdjustment(value);
        listener.height(heightAdjustment);
        updateHint("键盘高度 " + KeyboardGeometry.displayHeight(heightAdjustment));
        // Repeated steps coalesce into one save, the way a drag saves once when the finger lifts.
        // Saving on every step loses saves instead: a save already in flight makes the next
        // request a no-op, so the last steps -- and a reset tapped straight afterwards -- vanish.
        removeCallbacks(commitAdjustment);
        postDelayed(commitAdjustment, COMMIT_DELAY_MILLIS);
        return true;
    }

    private void commitAdjustment() { listener.commit(); }

    @Override protected void onDetachedFromWindow() {
        removeCallbacks(commitAdjustment);
        super.onDetachedFromWindow();
    }

    @SuppressWarnings("deprecation")
    @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
        super.onInitializeAccessibilityNodeInfo(info);
        info.setClassName("android.widget.SeekBar");
        info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(
            AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_INT,
            KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_DP,
            KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_DP,
            heightAdjustment));
    }
}
