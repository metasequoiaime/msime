package app.msime.android;

import android.graphics.Color;
import android.widget.Button;
import android.widget.LinearLayout;

/** 临时拼写编辑表面：一行放大拼写加「完成」；组字与光标状态仍由引擎持有。 */
final class ImeCompositionEditor {
    private final MSIMEInputService s;
    private LinearLayout container;
    private CompositionEditorView spelling;
    private boolean open;

    ImeCompositionEditor(MSIMEInputService service) { s = service; }

    void install(LinearLayout parent) {
        // 只占一行，拼写条在左、「完成」在右，不再另起提示行和手柄区把键盘顶高。
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

    void open() {
        if (!s.compositionEditable()) return;
        open = true;
        s.render();
    }

    void close() {
        open = false;
        if (container != null) ViewPolicy.hide(container);
    }

    void render(String display, String editing, int caret) {
        if (!s.compositionEditable()) close();
        if (container == null) return;
        ViewPolicy.setVisible(container, open);
        if (!open) return;
        container.setBackgroundColor(Color.parseColor(s.skin.background()));
        // 读音无法与原始按键对齐时，显示可编辑的按键，不猜测位置。
        if (CompositionCaretPolicy.displayOffset(display, editing, caret) < 0) display = editing;
        spelling.update(display, editing, caret, Color.parseColor(s.skin.keyForeground()),
            Color.parseColor(s.skin.accent()));
    }
}
