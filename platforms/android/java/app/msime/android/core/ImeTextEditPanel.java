package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.graphics.Typeface;
import android.os.SystemClock;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.FrameLayout;
import java.util.EnumMap;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONObject;
import org.json.JSONException;

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
    private TextTrackpadSettings trackpadSettings;
    private boolean liveTrackpadPreferencesLoaded;
    private TextTrackpadView trackpad;
    private LinearLayout arrows;
    private Button modeButton;
    private Button trackpadSelect;
    private boolean modeSaving;
    private boolean destroyed;
    private long modeSaveGeneration;
    private long selectionVersion;
    private int selectionStart = -1;
    private int selectionEnd = -1;
    private boolean previewActive;
    private InputConnection previewTarget;
    private android.graphics.RectF previewAnchor;
    private TextTrackpadText.Preview previewText;
    private TextTrackpadMagnifier magnifier;
    private boolean temporaryTrackpad;
    private boolean previouslyOpen;
    private boolean previousSelecting;
    private ShortcutTouch shortcutTouch;
    private final ExecutorService trackpadWorker = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-text-trackpad"));
    private final TextTrackpadReadQueue<TextTrackpadText.Range> selectionReads;
    private final TextTrackpadReadQueue<TextTrackpadText.Preview> previewReads;

    ImeTextEditPanel(MSIMEInputService s) {
        this.s = s;
        selectionReads = new TextTrackpadReadQueue<>(trackpadWorker, s.main::post);
        previewReads = new TextTrackpadReadQueue<>(trackpadWorker, s.main::post);
    }

    /** 建面板视图（隐藏状态），布局与 {@link TextEditPanelModel} 的 4 × 4 网格一致。 */
    LinearLayout build() {
        if (shortcutTouch != null) shortcutTouch.cancel(false);
        setPreviewActive(false);
        if (trackpad != null) trackpad.cancel();
        buttons.clear();
        LinearLayout panel = LayoutPolicy.row(s);
        panel.setContentDescription("文本编辑面板");
        int padding = s.pixels(4);
        ViewPolicy.setPadding(panel, padding, padding, padding, padding);
        // 键之间的空隙不能把触摸漏给底下的字母键。
        ViewPolicy.setClickable(panel, true);

        LinearLayout left = LayoutPolicy.column(s);
        modeButton = s.keyboardKey("触控板", "切换为触控板", this::toggleTrackpad);
        left.addView(modeButton, new LinearLayout.LayoutParams(-1, s.pixels(32)));
        arrows = LayoutPolicy.row(s);
        arrows.addView(key(TextEditPanelModel.Action.LEFT), cell(true));
        LinearLayout vertical = LayoutPolicy.column(s);
        vertical.addView(key(TextEditPanelModel.Action.UP), cell(false));
        vertical.addView(key(TextEditPanelModel.Action.SELECT), cell(false));
        vertical.addView(key(TextEditPanelModel.Action.DOWN), cell(false));
        arrows.addView(vertical, LayoutPolicy.weightedMatchParentParams(1));
        arrows.addView(key(TextEditPanelModel.Action.RIGHT), cell(true));
        FrameLayout movement = new FrameLayout(s);
        movement.addView(arrows, new FrameLayout.LayoutParams(-1, -1));
        trackpad = new TextTrackpadView(s, new TextTrackpadView.Listener() {
            @Override public Object target() { return s.connection; }
            @Override public boolean move(TextTrackpadGesture.Steps steps) { return moveTrackpad(steps); }
            @Override public void select(TextTrackpadGesture.Tap tap) { selectTrackpad(tap); }
            @Override public void active(boolean active) {
                if (active) {
                    selectionReads.cancel();
                    selectionVersion++;
                }
                setPreviewActive(active);
            }
        });
        movement.addView(trackpad, new FrameLayout.LayoutParams(-1, -1));
        left.addView(movement, LayoutPolicy.weightedWidthParams(3));
        LinearLayout ends = LayoutPolicy.row(s);
        ends.addView(key(TextEditPanelModel.Action.DOCUMENT_START), cell(true));
        ends.addView(key(TextEditPanelModel.Action.DOCUMENT_END), cell(true));
        ends.addView(key(TextEditPanelModel.Action.DELETE), cell(true));
        trackpadSelect = s.keyboardKey("选择", "选择模式", () -> perform(TextEditPanelModel.Action.SELECT));
        ends.addView(trackpadSelect, cell(true));
        left.addView(ends, LayoutPolicy.weightedWidthParams(1));
        panel.addView(left, LayoutPolicy.weightedMatchParentParams(3));

        LinearLayout clipboard = LayoutPolicy.column(s);
        clipboard.addView(key(TextEditPanelModel.Action.SELECT_ALL), cell(false));
        clipboard.addView(key(TextEditPanelModel.Action.COPY), cell(false));
        clipboard.addView(key(TextEditPanelModel.Action.CUT), cell(false));
        clipboard.addView(key(TextEditPanelModel.Action.PASTE), cell(false));
        for (TextEditPanelModel.Key shortcut : TextEditPanelModel.shortcuts())
            clipboard.addView(key(shortcut.action()), cell(false));
        ScrollView scroll = new ScrollView(s);
        scroll.setContentDescription("文本编辑快捷操作");
        scroll.setFillViewport(true);
        scroll.setVerticalScrollBarEnabled(false);
        scroll.setOverScrollMode(View.OVER_SCROLL_NEVER);
        scroll.addView(clipboard, LayoutPolicy.matchParentParams());
        // 右列保持四行可见，新增的三个快捷键通过上下滑动访问；按键高度跟随当前键盘高度调整。
        scroll.addOnLayoutChangeListener((view, xLeft, yTop, xRight, yBottom,
                oldLeft, oldTop, oldRight, oldBottom) -> resizeShortcutRows(clipboard, yBottom - yTop));
        panel.addView(scroll, LayoutPolicy.weightedMatchParentParams(1));
        ViewPolicy.hide(panel);
        configureTrackpad();
        return panel;
    }

    private void resizeShortcutRows(LinearLayout column, int viewportHeight) {
        int height = viewportHeight / 4;
        if (height <= 0) return;
        for (int index = 0; index < column.getChildCount(); index++) {
            View child = column.getChildAt(index);
            LinearLayout.LayoutParams params = (LinearLayout.LayoutParams) child.getLayoutParams();
            if (params.height == height && params.weight == 0) continue;
            params.width = LinearLayout.LayoutParams.MATCH_PARENT;
            params.height = height;
            params.weight = 0;
            child.setLayoutParams(params);
        }
    }

    /** 一格的布局参数：`horizontal` 为真时在横排里按宽度均分，否则在竖排里按高度均分，四周留出键距。 */
    private LinearLayout.LayoutParams cell(boolean horizontal) {
        LinearLayout.LayoutParams params = horizontal
            ? LayoutPolicy.weightedMatchParentParams(1) : LayoutPolicy.weightedWidthParams(1);
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
            if (isArrow(action)) {
                // 系统默认按钮字号和字重让方向箭头显得细小；编辑面板的箭头是主要操作，统一放大并加粗。
                KeyboardTypographyPolicy.setKeyTextSize(button, isDocumentEdge(action) ? 36 : 24);
                button.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
            } else if (action == TextEditPanelModel.Action.SELECT || spec.label().length() > 1)
                KeyboardTypographyPolicy.setKeyTextSize(button, 16);
            if (spec.repeats()) bindRepeat(button, run);
        }
        ViewPolicy.clearPadding(button);
        button.setContentDescription(spec.description());
        buttons.put(action, button);
        return button;
    }

    private static boolean isArrow(TextEditPanelModel.Action action) {
        return action == TextEditPanelModel.Action.LEFT
            || action == TextEditPanelModel.Action.UP
            || action == TextEditPanelModel.Action.DOWN
            || action == TextEditPanelModel.Action.RIGHT
            || action == TextEditPanelModel.Action.DOCUMENT_START
            || action == TextEditPanelModel.Action.DOCUMENT_END;
    }

    /** `⇤` / `⇥` 的字面框比方向箭头窄，同字号绘制时视觉尺寸明显偏小。 */
    private static boolean isDocumentEdge(TextEditPanelModel.Action action) {
        return action == TextEditPanelModel.Action.DOCUMENT_START
            || action == TextEditPanelModel.Action.DOCUMENT_END;
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
        configureTrackpad();
        ViewPolicy.show(s.textEditPanel);
    }

    void close() {
        if (shortcutTouch != null) shortcutTouch.cancel(false);
        temporaryTrackpad = false;
        stopRepeat();
        selectionReads.cancel();
        if (trackpad != null) trackpad.cancel();
        setPreviewActive(false);
        selecting = false;
        if (s.textEditPanel != null) ViewPolicy.hide(s.textEditPanel);
    }

    private void renderState() {
        Button select = buttons.get(TextEditPanelModel.Action.SELECT);
        if (select == null) return;
        select.setText(selecting ? "取消选择" : "选择");
        select.setContentDescription(selecting ? "取消选择" : "选择模式");
        ViewPolicy.setSelected(select, selecting);
        select.setActivated(selecting);
        if (trackpadSelect != null) {
            trackpadSelect.setText(selecting ? "取消选择" : "选择");
            trackpadSelect.setContentDescription(selecting ? "取消选择" : "选择模式");
            ViewPolicy.setSelected(trackpadSelect, selecting);
            trackpadSelect.setActivated(selecting);
        }
        if (android.os.Build.VERSION.SDK_INT >= 30)
            select.setStateDescription(selecting ? "已开启，方向键延伸选区" : "已关闭");
    }

    private void perform(TextEditPanelModel.Action action) {
        InputConnection target = s.connection;
        if (target == null) return;
        s.selectionEcho.invalidate();
        if (action == TextEditPanelModel.Action.SELECT) {
            selecting = !selecting;
        } else if (action == TextEditPanelModel.Action.NUMBER_PAD) {
            selecting = false;
            s.showNumberKeyboardFromTextEdit();
            return;
        } else if (action == TextEditPanelModel.Action.SPACE) {
            InputConnectionGuard.commitText(target, " ", 1);
            selecting = false;
        } else if (action == TextEditPanelModel.Action.RETURN) {
            InputConnectionGuard.commitText(target, "\n", 1);
            selecting = false;
        } else {
            int menu = TextEditPanelModel.contextMenuAction(action);
            if (menu != 0) {
                boolean done = InputConnectionGuard.performContextMenuAction(target, menu);
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

    boolean canSwipeCopyAll() {
        return s.connection != null && !EditorPolicy.password(s.editorInputType) && !s.hasEngineComposition()
            && (s.handwritingCanvas == null || !s.handwritingCanvas.hasInk());
    }

    /** 复制始终交给编辑器；提取结果只保留选区元数据，用于核对全选和恢复原位置。 */
    void copyAllFromGesture() {
        if (!canSwipeCopyAll()) return;
        InputConnection target = s.connection;
        s.selectionEcho.invalidate();
        boolean copied = EditorCopyAll.copy(new EditorCopyAll.Editor() {
            @Override public Object target() { return s.connection; }
            @Override public EditorCopyAll.Selection read() {
                try {
                    var request = new android.view.inputmethod.ExtractedTextRequest();
                    var text = target.getExtractedText(request, 0);
                    if (text == null || text.text == null || text.startOffset != 0 || text.partialStartOffset != -1)
                        return null;
                    var selection = new EditorCopyAll.Selection(text.selectionStart, text.selectionEnd, text.text.length());
                    return selection.valid() ? selection : null;
                } catch (RuntimeException ignored) { return null; }
            }
            @Override public boolean selectAll() {
                return InputConnectionGuard.performContextMenuAction(target, android.R.id.selectAll);
            }
            @Override public boolean copy() {
                return InputConnectionGuard.performContextMenuAction(target, android.R.id.copy);
            }
            @Override public boolean select(int start, int end) {
                try { return target.setSelection(start, end); }
                catch (RuntimeException ignored) { return false; }
            }
        });
        if (!copied) s.notice("当前输入框不支持复制全文");
    }

    /**
     * 发一次按键。带 Shift 时先按下、最后松开左 Shift 键：EditText 只在文字缓冲里记着 Shift 按下时才延伸选区（ArrowKeyMovementMethod.isSelecting 看的是缓冲的修饰键状态，不看事件自己的 meta），WebView 则看事件的 meta，两者都给。
     */
    private boolean sendKey(InputConnection target, int keyCode, int metaState) {
        boolean shift = (metaState & KeyEvent.META_SHIFT_ON) != 0;
        long down = SystemClock.uptimeMillis();
        boolean done = !shift || InputConnectionGuard.sendKeyEvent(target, event(down, down, KeyEvent.ACTION_DOWN,
            KeyEvent.KEYCODE_SHIFT_LEFT, KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON));
        if (done) done = InputConnectionGuard.sendKeyEvent(target, event(down, down, KeyEvent.ACTION_DOWN, keyCode, metaState));
        boolean released = InputConnectionGuard.sendKeyEvent(target, event(down, SystemClock.uptimeMillis(), KeyEvent.ACTION_UP, keyCode, metaState));
        if (shift) InputConnectionGuard.sendKeyEvent(target, event(down, SystemClock.uptimeMillis(), KeyEvent.ACTION_UP,
            KeyEvent.KEYCODE_SHIFT_LEFT, 0));
        return done && released;
    }

    void applyPreferences(JSONObject preferences) {
        applyPreferences(preferences, true);
    }

    void applyPreferences(JSONObject preferences, boolean live) {
        if (!live && liveTrackpadPreferencesLoaded) return;
        TextTrackpadSettings next = TextTrackpadSettings.from(preferences);
        if (live && next != null) liveTrackpadPreferencesLoaded = true;
        if (java.util.Objects.equals(next, trackpadSettings)) return;
        if (shortcutTouch != null) shortcutTouch.cancel(true);
        trackpadSettings = next;
        selectionReads.cancel();
        setPreviewActive(false);
        configureTrackpad();
    }

    private void configureTrackpad() {
        if (trackpad == null) return;
        if (trackpadSettings != null) {
            trackpad.configure(trackpadSettings.horizontalDp(), trackpadSettings.verticalDp(),
                trackpadSettings.tapSelection(), trackpadSettings.trail(),
                android.graphics.Color.parseColor(s.skin.background()),
                android.graphics.Color.parseColor(s.skin.keyForeground()), android.graphics.Color.parseColor(s.skin.accent()));
        }
        boolean enabled = trackpadSettings != null && (trackpadSettings.enabled() || temporaryTrackpad);
        ViewPolicy.setVisible(trackpad, enabled);
        ViewPolicy.setVisible(arrows, !enabled);
        ViewPolicy.setVisible(trackpadSelect, enabled);
        modeButton.setText(enabled ? "方向键" : "触控板");
        modeButton.setContentDescription(enabled ? "切换为方向键" : "切换为触控板");
        ViewPolicy.setEnabled(modeButton, trackpadSettings != null && !modeSaving && !temporaryTrackpad
            && !preferencesDirectory().isEmpty());
    }

    /** 触摸仍归原工具按钮，越过按钮边界后显式转交触控板，避免丢掉松手事件。 */
    void bindShortcut(Button button) {
        if (shortcutTouch != null) shortcutTouch.cancel(false);
        shortcutTouch = new ShortcutTouch(button);
        button.setOnTouchListener(shortcutTouch);
    }

    private final class ShortcutTouch implements View.OnTouchListener {
        private final Button button;
        private final TextTrackpadHold hold;
        private final Runnable activate;
        private boolean owned;
        private boolean forwarded;
        private long forwardedAt;

        ShortcutTouch(Button button) {
            this.button = button;
            hold = new TextTrackpadHold(android.view.ViewConfiguration.getLongPressTimeout(),
                android.view.ViewConfiguration.get(s).getScaledTouchSlop());
            activate = () -> {
                if (!owned || !hold.activate(s.connection, SystemClock.uptimeMillis())) return;
                if (s.textEditPanel == null || trackpad == null || trackpadSettings == null) {
                    cancel(false);
                    return;
                }
                if (s.session != 0 && s.hasEngineComposition()) s.command(2);
                previouslyOpen = ViewPolicy.isVisible(s.textEditPanel);
                previousSelecting = selecting;
                temporaryTrackpad = true;
                selecting = false;
                selectionReads.cancel();
                stopRepeat();
                configureTrackpad();
                renderState();
                s.imeStyler.applySkinBackground(s.textEditPanel);
                ViewPolicy.show(s.textEditPanel);
                s.textEditPanel.bringToFront();
                s.render();
                s.imeKeyFeedback.playFeedback(button);
            };
        }

        @Override public boolean onTouch(View source, MotionEvent event) {
            int action = event.getActionMasked();
            if (action == MotionEvent.ACTION_DOWN) {
                if (trackpadSettings == null || !trackpadSettings.holdToOpen()) return false;
                cancel(true);
                owned = hold.down(s.connection, event.getX(), event.getY(), event.getEventTime());
                if (!owned) return false;
                button.setPressed(true);
                button.getParent().requestDisallowInterceptTouchEvent(true);
                button.postDelayed(activate, android.view.ViewConfiguration.getLongPressTimeout());
                return true;
            }
            if (!owned) return false;
            if (event.getPointerCount() != 1 || action == MotionEvent.ACTION_CANCEL
                    || action == MotionEvent.ACTION_OUTSIDE) {
                cancel(true);
                return true;
            }
            if (action == MotionEvent.ACTION_MOVE) {
                if (!hold.move(s.connection, event.getPointerCount(), event.getX(), event.getY())) cancel(true);
                else if (hold.active()) forward(event);
            } else if (action == MotionEvent.ACTION_UP) {
                if (hold.active()) forward(event);
                TextTrackpadHold.End end = hold.up(s.connection);
                finish(end, true);
                if (end == TextTrackpadHold.End.TAP) button.performClick();
            }
            return true;
        }

        private void forward(MotionEvent event) {
            if (trackpad == null || !ViewPolicy.isVisible(trackpad)) return;
            int[] location = new int[2];
            trackpad.getLocationOnScreen(location);
            float x = event.getRawX() - location[0];
            float y = event.getRawY() - location[1];
            if (!forwarded) {
                if (event.getActionMasked() == MotionEvent.ACTION_UP || x < 0 || y < 0
                        || x >= trackpad.getWidth() || y >= trackpad.getHeight()) return;
                forwardedAt = event.getEventTime();
                forwarded = true;
            }
            int action = event.getActionMasked();
            if (forwardedAt == event.getEventTime() && action == MotionEvent.ACTION_MOVE) action = MotionEvent.ACTION_DOWN;
            MotionEvent local = MotionEvent.obtain(forwardedAt, event.getEventTime(), action, x, y, 0);
            try { trackpad.dispatchTouchEvent(local); }
            finally { local.recycle(); }
        }

        void cancel(boolean restore) { finish(hold.cancel(), restore); }

        private void finish(TextTrackpadHold.End end, boolean restore) {
            owned = false;
            button.removeCallbacks(activate);
            button.setPressed(false);
            forwarded = false;
            if (end != TextTrackpadHold.End.RESTORE && !temporaryTrackpad) return;
            if (trackpad != null) trackpad.cancel();
            temporaryTrackpad = false;
            selecting = restore && previousSelecting;
            configureTrackpad();
            renderState();
            if (s.textEditPanel != null && (!restore || !previouslyOpen)) ViewPolicy.hide(s.textEditPanel);
            if (restore) s.render();
        }
    }

    private void toggleTrackpad() {
        if (destroyed || modeSaving || trackpadSettings == null || preferencesDirectory().isEmpty()) return;
        boolean enabled = !trackpadSettings.enabled();
        String directory = preferencesDirectory();
        long operation = ++modeSaveGeneration;
        modeSaving = true;
        ViewPolicy.setEnabled(modeButton, false);
        s.preferencesWorker.execute(() -> {
            JSONObject saved = null;
            try {
                JSONObject pending = s.nativeValue(NativeClient.loadPreferences(directory));
                pending.getJSONObject("preferences").getJSONObject("android_text_trackpad").put("enabled", enabled);
                saved = s.nativeValue(NativeClient.savePreferences(directory, pending.getLong("revision"), pending.toString()));
            } catch (JSONException | RuntimeException | LinkageError ignored) { }
            JSONObject result = saved;
            s.main.post(() -> {
                if (destroyed || operation != modeSaveGeneration) return;
                modeSaving = false;
                if (!directory.equals(preferencesDirectory())) {
                    configureTrackpad();
                    return;
                }
                if (result == null) s.notice("触控板设置未能保存，请重试");
                else s.acceptTextTrackpadPreferences(result);
                configureTrackpad();
            });
        });
    }

    private String preferencesDirectory() {
        return s.preferencesDirectory;
    }

    private boolean moveTrackpad(TextTrackpadGesture.Steps steps) {
        InputConnection target = s.connection;
        if (target == null) return false;
        s.selectionEcho.invalidate();
        TextEditPanelModel.Action horizontal = steps.horizontal() < 0 ? TextEditPanelModel.Action.LEFT : TextEditPanelModel.Action.RIGHT;
        TextEditPanelModel.Action vertical = steps.vertical() < 0 ? TextEditPanelModel.Action.UP : TextEditPanelModel.Action.DOWN;
        for (TextEditPanelModel.Action action : new TextEditPanelModel.Action[] {horizontal, vertical}) {
            int count = action == horizontal ? Math.abs(steps.horizontal()) : Math.abs(steps.vertical());
            for (int index = 0; index < count; index++) {
                if (s.connection != target || !sendKey(target, TextEditPanelModel.keyCode(action),
                        TextEditPanelModel.metaState(action, selecting || steps.selecting()))) return false;
                if (trackpadSettings != null && trackpadSettings.haptics() && s.hapticsEnabled)
                    trackpad.performHapticFeedback(android.view.HapticFeedbackConstants.KEYBOARD_TAP);
            }
        }
        refreshPreview();
        return true;
    }

    void editorChanged(int start, int end) {
        close();
        selectionChanged(start, end);
    }

    void selectionChanged(int start, int end) {
        selectionVersion++;
        selectionStart = start;
        selectionEnd = end;
        refreshPreview();
    }

    boolean needsCursorUpdates() {
        return previewActive && previewTarget != null && previewTarget == s.connection;
    }

    private void setPreviewActive(boolean active) {
        boolean wanted = active && trackpadSettings != null && trackpadSettings.magnifier()
            && s.connection != null;
        if (wanted && previewActive && previewTarget == s.connection) return;
        previewReads.cancel();
        previewText = null;
        previewAnchor = null;
        previewActive = wanted;
        previewTarget = wanted ? s.connection : null;
        if (!wanted && magnifier != null) magnifier.hide();
        s.setTextTrackpadPreviewExpanded(wanted && android.os.Build.VERSION.SDK_INT < 30);
        s.synchronizeCursorUpdates();
        if (wanted) refreshPreview();
    }

    private void refreshPreview() {
        if (!needsCursorUpdates()) return;
        InputConnection target = previewTarget;
        boolean password = EditorPolicy.password(s.editorInputType);
        int length = selectionStart < 0 || selectionEnd < 0 ? -1 : Math.abs(selectionEnd - selectionStart);
        boolean reverse = selectionEnd < selectionStart;
        previewReads.request(() -> TextTrackpadText.readPreview(
            limit -> target.getTextBeforeCursor(limit, 0), limit -> target.getTextAfterCursor(limit, 0),
            length, password), value -> {
                if (!needsCursorUpdates() || previewTarget != target || !ViewPolicy.isVisible(s.textEditPanel)) return;
                previewText = value == null ? null : value.direction(reverse);
                showPreview();
            });
    }

    void cursorAnchor(android.view.inputmethod.CursorAnchorInfo info) {
        if (!needsCursorUpdates()) return;
        previewAnchor = null;
        if (info != null && (info.getInsertionMarkerFlags()
                & android.view.inputmethod.CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION) != 0) {
            android.graphics.RectF bounds = new android.graphics.RectF(info.getInsertionMarkerHorizontal(),
                info.getInsertionMarkerTop(), info.getInsertionMarkerHorizontal(), info.getInsertionMarkerBottom());
            info.getMatrix().mapRect(bounds);
            if (Float.isFinite(bounds.left) && Float.isFinite(bounds.right)
                    && Float.isFinite(bounds.top) && Float.isFinite(bounds.bottom)
                    && bounds.top <= bounds.bottom) previewAnchor = bounds;
        }
        if (previewText != null) showPreview();
    }

    private void showPreview() {
        if (magnifier == null) magnifier = new TextTrackpadMagnifier(s);
        magnifier.show(s.keyboardSurface, previewAnchor, previewText,
            android.graphics.Color.parseColor(s.skin.background()), android.graphics.Color.parseColor(s.skin.keyForeground()),
            android.graphics.Color.parseColor(s.skin.accent()));
    }

    private void selectTrackpad(TextTrackpadGesture.Tap tap) {
        InputConnection target = s.connection;
        if (target == null || EditorPolicy.password(s.editorInputType)) return;
        long version = selectionVersion;
        selectionReads.request(() -> readTrackpadSelection(target, tap), range -> {
            if (s.connection != target || version != selectionVersion || !ViewPolicy.isVisible(s.textEditPanel)) return;
            if (range == null) {
                s.notice("当前编辑器未提供完整的词或行，仍可长按拖动选择");
                return;
            }
            s.selectionEcho.invalidate();
            try {
                if (!target.setSelection(range.start(), range.end())) s.notice("当前输入框不支持选择");
            }
            catch (RuntimeException ignored) { s.notice("当前输入框不支持选择"); }
        });
    }

    private TextTrackpadText.Range readTrackpadSelection(InputConnection target, TextTrackpadGesture.Tap tap) {
        try {
            var request = new android.view.inputmethod.ExtractedTextRequest();
            request.hintMaxChars = TextTrackpadText.SELECTION_CHARS;
            var extracted = target.getExtractedText(request, 0);
            if (extracted == null || extracted.text == null || extracted.startOffset < 0
                    || extracted.partialStartOffset != -1 || extracted.selectionStart < 0 || extracted.selectionEnd < 0
                    || extracted.selectionStart > extracted.text.length() || extracted.selectionEnd > extracted.text.length()
                    || (long) extracted.startOffset + extracted.text.length() > Integer.MAX_VALUE) return null;
            int focus = extracted.selectionEnd;
            int from = Math.max(0, focus - TextTrackpadText.SELECTION_CHARS / 2);
            int to = Math.min(extracted.text.length(), from + TextTrackpadText.SELECTION_CHARS);
            String text = extracted.text.subSequence(from, to).toString();
            int tail = Math.max(extracted.selectionStart, extracted.selectionEnd);
            int remaining = extracted.text.length() - tail;
            boolean ends = to == extracted.text.length() && remaining <= TextTrackpadText.SELECTION_CHARS
                && TextTrackpadText.endsDocument(extracted.text, tail, target.getTextAfterCursor(remaining + 1, 0));
            boolean begins = extracted.startOffset == 0 && from == 0;
            int offset = extracted.startOffset + from;
            int caret = extracted.startOffset + focus;
            return tap == TextTrackpadGesture.Tap.WORD
                ? TextTrackpadText.word(text, offset, caret, begins, ends, java.util.Locale.getDefault())
                : TextTrackpadText.line(text, offset, caret, begins, ends);
        } catch (RuntimeException ignored) { return null; }
    }

    void shutdown() {
        destroyed = true;
        modeSaveGeneration++;
        close();
        selectionReads.close();
        previewReads.close();
        trackpadWorker.shutdownNow();
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
        if (text != null && text.length() > 0) InputConnectionGuard.commitText(target, text, 1);
    }
}
