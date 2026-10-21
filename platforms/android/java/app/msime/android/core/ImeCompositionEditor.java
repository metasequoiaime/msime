package app.msime.android;

import android.graphics.Color;
import android.widget.Button;
import android.widget.LinearLayout;

/** 临时拼写编辑表面；组字与光标状态仍由引擎持有。 */
final class ImeCompositionEditor {
    private final MSIMEInputService s;
    private LinearLayout container;
    private CompositionEditorView spelling;
    private boolean open;

    ImeCompositionEditor(MSIMEInputService service) { s = service; }

    void install(LinearLayout parent) {
        container = new LinearLayout(s);
        container.setOrientation(LinearLayout.VERTICAL);
        container.setContentDescription("拼写编辑面板");
        LinearLayout row = LayoutPolicy.row(s);
        row.addView(ViewPolicy.textLabel(s, "拖动右侧手柄移动光标", 12),
            new LinearLayout.LayoutParams(0, s.pixels(40), 1));
        Button done = new Button(s);
        done.setText("完成");
        done.setContentDescription("完成拼写编辑");
        done.setOnClickListener(ignored -> {
            close();
            s.command(CompositionCaretPolicy.MOVE_END);
        });
        row.addView(done, new LinearLayout.LayoutParams(s.pixels(64), s.pixels(40)));
        container.addView(row);
        spelling = new CompositionEditorView(s, s::moveCompositionCaret);
        container.addView(spelling, LayoutPolicy.matchWidthWrapParams());
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
