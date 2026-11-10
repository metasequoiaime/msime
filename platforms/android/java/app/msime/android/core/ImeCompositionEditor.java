package app.msime.android;

import android.graphics.Color;
import android.view.View;
import android.widget.FrameLayout;
import android.widget.Button;
import android.widget.LinearLayout;

/** 临时拼写编辑表面：一行放大拼写加「完成」；组字与光标状态仍由引擎持有。 */
final class ImeCompositionEditor {
    private final MSIMEInputService s;
    private LinearLayout container;
    private CompositionEditorView spelling;
    private boolean open;

    ImeCompositionEditor(MSIMEInputService service) { s = service; }

    void install(FrameLayout parent) {
        // 只占一行，拼写条在左、「完成」在右；作为覆盖层时不参与候选区测量。
        container = LayoutPolicy.row(s);
        container.setGravity(android.view.Gravity.CENTER_VERTICAL);
        container.setPadding(s.pixels(8), s.pixels(4), s.pixels(8), s.pixels(4));
        container.setContentDescription("拼写编辑面板");
        spelling = new CompositionEditorView(s, s::moveCompositionCaret);
        container.addView(spelling, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        Button done = new Button(s);
        done.setText("完成");
        done.setContentDescription("完成拼写编辑");
        done.setOnClickListener(ignored -> {
            close();
            s.command(CompositionCaretPolicy.MOVE_END);
        });
        LinearLayout.LayoutParams doneParams = new LinearLayout.LayoutParams(s.pixels(64), s.pixels(40));
        doneParams.setMarginStart(s.pixels(8));
        container.addView(done, doneParams);
        parent.addView(container, LayoutPolicy.matchWidthWrapParams());
        ViewPolicy.hide(container);
    }

    /** 将覆盖层对齐到候选区顶部，避免显示或隐藏它时改变输入法窗口高度。 */
    void alignTopTo(View region) {
        if (container == null || region == null
                || !(container.getLayoutParams() instanceof FrameLayout.LayoutParams params)) return;
        View parent = (View) region.getParent();
        int top = (parent == null ? 0 : parent.getTop()) + region.getTop();
        if (params.topMargin == top) return;
        params.topMargin = top;
        container.post(() -> container.setLayoutParams(params));
    }

    void open() {
        if (!s.compositionEditable()) return;
        open = true;
        s.render();
    }

    void close() {
        open = false;
        if (container != null) ViewPolicy.hide(container);
    }

    /** @param keypad 这次组字所在键盘的组码表（{@link CompositionCaretPolicy#keypad}），读音的字母按它对上按键 */
    void render(String display, String editing, String keypad, int caret, boolean keyGrid) {
        if (!s.compositionEditable()) close();
        if (container == null) return;
        ViewPolicy.setVisible(container, open);
        if (!open) return;
        container.setBackgroundColor(Color.parseColor(s.skin.background()));
        // 读音无法与原始按键对齐时，显示可编辑的按键，不猜测位置。
        if (CompositionCaretPolicy.displayOffset(display, editing, keypad, caret, keyGrid) < 0) display = editing;
        spelling.update(display, editing, keypad, caret, keyGrid, Color.parseColor(s.skin.keyForeground()),
            Color.parseColor(s.skin.accent()));
    }
}
