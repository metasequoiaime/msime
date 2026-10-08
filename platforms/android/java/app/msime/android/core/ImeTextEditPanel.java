package app.msime.android;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.os.SystemClock;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import android.widget.LinearLayout;
import java.util.EnumMap;
import java.util.Map;

/**
 * 文本编辑面板（#5625）：键与动作见 {@link TextEditPanelModel}，这里负责建视图、把动作交给编辑器。和常用语、剪贴板面板一样盖在键区上、顶边对齐工具栏下沿，从功能面板的「文本编辑」打开，工具栏的收起键（此时是「返回键盘」）或再点一次功能面板回到键盘。
 *
 * <p>所有动作都直接作用于编辑器：打开面板前先由 Engine 完成组字，面板开着时不经过 Engine。方向键和删除按住连发，节奏与删除键相同（{@link BackspaceRepeatPolicy}）；删除键还带上滑快速删除。
 */
final class ImeTextEditPanel {
    private final MSIMEInputService s;
    private final Map<TextEditPanelModel.Action, Button> buttons =
        new EnumMap<>(TextEditPanelModel.Action.class);
    private boolean selecting;
    private Runnable repeatTask;

    ImeTextEditPanel(MSIMEInputService s) {
        this.s = s;
    }

    /** 建面板视图（隐藏状态），布局与 {@link TextEditPanelModel} 的 4 × 4 网格一致。 */
    LinearLayout build() {
        buttons.clear();
        LinearLayout panel = KeyboardGeometry.row(s);
        panel.setContentDescription("文本编辑面板");
        int padding = s.pixels(4);
        ViewPolicy.setPadding(panel, padding, padding, padding, padding);
        // 键之间的空隙不能把触摸漏给底下的字母键。
        ViewPolicy.setClickable(panel, true);

        LinearLayout left = KeyboardGeometry.column(s);
        LinearLayout arrows = KeyboardGeometry.row(s);
        arrows.addView(key(TextEditPanelModel.Action.LEFT), cell(true));
        LinearLayout vertical = KeyboardGeometry.column(s);
        vertical.addView(key(TextEditPanelModel.Action.UP), cell(false));
        vertical.addView(key(TextEditPanelModel.Action.SELECT), cell(false));
        vertical.addView(key(TextEditPanelModel.Action.DOWN), cell(false));
        arrows.addView(vertical, KeyboardGeometry.weightedMatchParentParams(1));
        arrows.addView(key(TextEditPanelModel.Action.RIGHT), cell(true));
        left.addView(arrows, KeyboardGeometry.weightedWidthParams(3));
        LinearLayout ends = KeyboardGeometry.row(s);
        ends.addView(key(TextEditPanelModel.Action.DOCUMENT_START), cell(true));
        ends.addView(key(TextEditPanelModel.Action.DOCUMENT_END), cell(true));
        ends.addView(key(TextEditPanelModel.Action.DELETE), cell(true));
        left.addView(ends, KeyboardGeometry.weightedWidthParams(1));
        panel.addView(left, KeyboardGeometry.weightedMatchParentParams(3));

        LinearLayout clipboard = KeyboardGeometry.column(s);
        clipboard.addView(key(TextEditPanelModel.Action.SELECT_ALL), cell(false));
        clipboard.addView(key(TextEditPanelModel.Action.COPY), cell(false));
        clipboard.addView(key(TextEditPanelModel.Action.CUT), cell(false));
        clipboard.addView(key(TextEditPanelModel.Action.PASTE), cell(false));
        panel.addView(clipboard, KeyboardGeometry.weightedMatchParentParams(1));
        ViewPolicy.hide(panel);
        return panel;
    }

    /** 一格的布局参数：`horizontal` 为真时在横排里按宽度均分，否则在竖排里按高度均分，四周留出键距。 */
    private LinearLayout.LayoutParams cell(boolean horizontal) {
        LinearLayout.LayoutParams params = horizontal
            ? KeyboardGeometry.weightedMatchParentParams(1) : KeyboardGeometry.weightedWidthParams(1);
        int margin = s.pixels(3);
        params.setMargins(margin, margin, margin, margin);
        return params;
    }

    private Button key(TextEditPanelModel.Action action) {
        TextEditPanelModel.Key spec = TextEditPanelModel.key(action);
        Runnable run = () -> perform(action);
        Button button;
        if (action == TextEditPanelModel.Action.DELETE) {
            button = s.backspaceKey(run);
            s.imeLetterRows.bindBackspaceRepeat(button, run);
        } else {
            button = s.keyboardKey(spec.label(), spec.description(), run);
            if (action == TextEditPanelModel.Action.SELECT || spec.label().length() > 1)
                KeyboardGeometry.setKeyTextSize(button, 16);
            if (spec.repeats()) bindRepeat(button, run);
        }
        ViewPolicy.clearPadding(button);
        button.setContentDescription(spec.description());
        buttons.put(action, button);
        return button;
    }

    /** 按下立即执行一次，按住按 {@link BackspaceRepeatPolicy} 的节奏连发，松手或滑出键停止。 */
    private void bindRepeat(Button button, Runnable action) {
        button.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    stopRepeat();
                    button.setPressed(true);
                    s.imeKeyFeedback.playFeedback(button);
                    action.run();
                    Runnable task = new Runnable() {
                        private int repeats;

                        @Override public void run() {
                            if (repeatTask != this || !button.isPressed()) return;
                            s.imeKeyFeedback.playFeedback(button);
                            action.run();
                            repeats++;
                            s.main.postDelayed(this, BackspaceRepeatPolicy.repeatInterval(repeats));
                        }
                    };
                    repeatTask = task;
                    s.main.postDelayed(task, BackspaceRepeatPolicy.INITIAL_DELAY_MS);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (event.getX() < 0 || event.getY() < 0
                            || event.getX() >= button.getWidth() || event.getY() >= button.getHeight()) {
                        stopRepeat();
                        button.setPressed(false);
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_OUTSIDE -> {
                    stopRepeat();
                    button.setPressed(false);
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    private void stopRepeat() {
        if (repeatTask != null) s.main.removeCallbacks(repeatTask);
        repeatTask = null;
    }

    /** 打开面板：先完成组字，关掉别的面板；没有编辑器时提示并不打开。 */
    void show() {
        if (s.connection == null || s.textEditPanel == null) {
            s.notice("当前没有可编辑的输入框");
            return;
        }
        if (s.session != 0 && s.hasEngineComposition()) s.command(2);
        s.closeToolbarPanels();
        s.closeCandidatePanel();
        selecting = false;
        renderState();
        s.imeStyler.applySkinBackground(s.textEditPanel);
        ViewPolicy.show(s.textEditPanel);
    }

    void close() {
        stopRepeat();
        selecting = false;
        if (s.textEditPanel != null) ViewPolicy.hide(s.textEditPanel);
    }

    private void renderState() {
        Button select = buttons.get(TextEditPanelModel.Action.SELECT);
        if (select == null) return;
        ViewPolicy.setSelected(select, selecting);
        select.setActivated(selecting);
        if (android.os.Build.VERSION.SDK_INT >= 30)
            select.setStateDescription(selecting ? "已开启，方向键延伸选区" : "已关闭");
    }

    private void perform(TextEditPanelModel.Action action) {
        InputConnection target = s.connection;
        if (target == null) return;
        s.selectionEcho.invalidate();
        if (action == TextEditPanelModel.Action.SELECT) {
            selecting = !selecting;
        } else {
            int menu = TextEditPanelModel.contextMenuAction(action);
            if (menu != 0) {
                boolean done = target.performContextMenuAction(menu);
                // 有些编辑器（部分 WebView、跨平台框架的输入框）不实现上下文菜单动作；粘贴还能退回到直接上屏剪贴板文字。复制和剪切不退回：那要宿主自己读选中的文字，会绕过密码框禁止复制的规则。
                if (!done && action == TextEditPanelModel.Action.PASTE) pasteClipboardText(target);
            } else {
                sendKey(target, TextEditPanelModel.keyCode(action),
                    TextEditPanelModel.metaState(action, selecting));
            }
            if (TextEditPanelModel.endsSelecting(action)) selecting = false;
        }
        renderState();
    }

    /**
     * 发一次按键。带 Shift 时先按下、最后松开左 Shift 键：EditText 只在文字缓冲里记着 Shift 按下时才延伸选区（ArrowKeyMovementMethod.isSelecting 看的是缓冲的修饰键状态，不看事件自己的 meta），WebView 则看事件的 meta，两者都给。
     */
    private void sendKey(InputConnection target, int keyCode, int metaState) {
        boolean shift = (metaState & KeyEvent.META_SHIFT_ON) != 0;
        long down = SystemClock.uptimeMillis();
        if (shift) target.sendKeyEvent(event(down, down, KeyEvent.ACTION_DOWN,
            KeyEvent.KEYCODE_SHIFT_LEFT, KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON));
        target.sendKeyEvent(event(down, down, KeyEvent.ACTION_DOWN, keyCode, metaState));
        target.sendKeyEvent(event(down, SystemClock.uptimeMillis(), KeyEvent.ACTION_UP, keyCode, metaState));
        if (shift) target.sendKeyEvent(event(down, SystemClock.uptimeMillis(), KeyEvent.ACTION_UP,
            KeyEvent.KEYCODE_SHIFT_LEFT, 0));
    }

    private static KeyEvent event(long downTime, long eventTime, int action, int keyCode, int metaState) {
        return new KeyEvent(downTime, eventTime, action, keyCode, 0, metaState,
            KeyCharacterMap.VIRTUAL_KEYBOARD, 0,
            KeyEvent.FLAG_SOFT_KEYBOARD | KeyEvent.FLAG_KEEP_TOUCH_MODE);
    }

    private void pasteClipboardText(InputConnection target) {
        ClipboardManager clipboard = s.getSystemService(ClipboardManager.class);
        ClipData clip = clipboard == null ? null : clipboard.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) return;
        CharSequence text = clip.getItemAt(0).coerceToText(s);
        if (text != null && text.length() > 0) target.commitText(text, 1);
    }
}
