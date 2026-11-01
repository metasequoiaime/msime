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
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import java.util.List;
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
    private Button modeButton;
    private Button trackpadSelect;
    private boolean modeSaving;
    private boolean destroyed;
    private long modeSaveGeneration;
    private final TextTrackpadSelectionVersion selectionVersion = new TextTrackpadSelectionVersion();
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

    /** 所有操作同屏可见；中央十字下排两角留白，父面板拦截空白处触摸。 */
    LinearLayout build() {
        LinearLayout panel = LayoutPolicy.column(s);
        panel.setContentDescription("文本编辑面板");
        int padding = s.pixels(4);
        ViewPolicy.setPadding(panel, padding, padding, padding, padding);
        ViewPolicy.setClickable(panel, true);
        populate(panel);
        ViewPolicy.hide(panel);
        configureTrackpad();
        return panel;
    }

    private void populate(LinearLayout panel) {
        if (shortcutTouch != null) shortcutTouch.cancel(false);
        setPreviewActive(false);
        selectionReads.cancel();
        if (trackpad != null) trackpad.cancel();
        stopRepeat();
        buttons.clear();
        panel.removeAllViews();
        LinearLayout modes = LayoutPolicy.row(s);
        modeButton = s.keyboardKey("切换为触控板", "切换为触控板", this::toggleTrackpad);
        // 模式行固定 32 dp 高，六个字折成两行时第二行会被裁掉；窄窗口（自由窗口、分屏）里只缩字号、保持一行。
        KeyboardTypographyPolicy.setFittedKeyTextSize(modeButton, 10, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
        modes.addView(modeButton, LayoutPolicy.weightedMatchParentParams(1));
        trackpadSelect = s.keyboardKey("选择", "选择模式", () -> perform(TextEditPanelModel.Action.SELECT));
        modes.addView(trackpadSelect, LayoutPolicy.weightedMatchParentParams(1));
        panel.addView(modes, LayoutPolicy.matchWidthHeightPx(s.pixels(32)));
        FrameLayout movement = new FrameLayout(s);
        LinearLayout grid = LayoutPolicy.column(s);
        movement.addView(grid, LayoutPolicy.frameMatchParentParams());
        populateGrid(grid);
        trackpad = new TextTrackpadView(s, new TextTrackpadView.Listener() {
            @Override public Object target() { return s.connection; }
            @Override public boolean move(TextTrackpadGesture.Steps steps) { return moveTrackpad(steps); }
            @Override public void select(TextTrackpadGesture.Tap tap) { selectTrackpad(tap); }
            @Override public void active(boolean active) {
                if (active) {
                    selectionReads.cancel();
                    selectionVersion.invalidate();
                }
                setPreviewActive(active);
            }
        });
        TextTrackpadView currentTrackpad = trackpad;
        movement.addView(currentTrackpad, new FrameLayout.LayoutParams(1, 1));
        // 触控板替代中央第二、三行；上排行首尾、两侧八键和底部首尾、换行保持可用。
        movement.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> {
            int width = right - left, height = bottom - top, margin = s.pixels(3);
            FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
                Math.max(1, width * 6 / 10 - 2 * margin), Math.max(1, height / 2 - 2 * margin));
            params.leftMargin = width * 2 / 10 + margin;
            params.topMargin = height / 4 + margin;
            currentTrackpad.setLayoutParams(params);
        });
        panel.addView(movement, LayoutPolicy.weightedWidthParams(1));
    }

    private void populateGrid(LinearLayout panel) {
        org.json.JSONObject preferences = s.preferencesSnapshot == null
            ? null : s.preferencesSnapshot.optJSONObject("preferences");
        List<TextEditPanelModel.Key> keys = TextEditPanelModel.keys(TextEditButtonOrder.fromPreferences(preferences));
        for (int rowIndex = 0; rowIndex < TextEditPanelModel.ROWS; rowIndex++) {
            LinearLayout row = LayoutPolicy.row(s);
            int column = 0;
            for (TextEditPanelModel.Key spec : keys) {
                if (spec.row() != rowIndex) continue;
                if (spec.column() > column) row.addView(new View(s), LayoutPolicy.weightedMatchParentParams(spec.column() - column));
                FrameLayout slot = new FrameLayout(s);
                FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT);
                int margin = s.pixels(3);
                params.setMargins(margin, margin, margin, margin);
                slot.addView(key(spec.action()), params);
                row.addView(slot, LayoutPolicy.weightedMatchParentParams(spec.columnSpan()));
                column = spec.column() + spec.columnSpan();
            }
            if (column < TextEditPanelModel.COLUMNS)
                row.addView(new View(s), LayoutPolicy.weightedMatchParentParams(TextEditPanelModel.COLUMNS - column));
            panel.addView(row, LayoutPolicy.weightedWidthParams(1));
        }
    }

    private Button key(TextEditPanelModel.Action action) {
        TextEditPanelModel.Key spec = TextEditPanelModel.key(action);
        Runnable run = () -> perform(action);
        Button button;
        if (action == TextEditPanelModel.Action.DELETE) {
            button = s.backspaceKey(run);
            s.imeLetterRows.bindBackspaceRepeat(button, run);
        } else if (action == TextEditPanelModel.Action.RETURN) {
            button = new TextEditReturnButton(s);
            button.setText(spec.label());
            s.imeStyler.styleButton(button, false);
            s.bindCountedAction(button, run);
            button.setOnLongClickListener(ignored -> {
                if (s.connection == null || s.session == 0 || s.symbolPanel == null
                        || s.textEditPanel == null || !s.textEditPanel.isShown()) return true;
                close();
                s.imePanels.showSymbolPanel();
                return true;
            });
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

    private static boolean isCursorMove(TextEditPanelModel.Action action) {
        return action == TextEditPanelModel.Action.LEFT || action == TextEditPanelModel.Action.UP
            || action == TextEditPanelModel.Action.DOWN || action == TextEditPanelModel.Action.RIGHT;
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
        populate(s.textEditPanel);
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
        selectionVersion.reset();
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
        // 键帽不是 state-list：styleButton 按调用那一刻的 isSelected() 装一块实心背景，开着时是皮肤强调色。面板开着时不经过引擎 render，也不会整树换肤，只改 selected 标志键面不会变色（#10454），所以这里立即重新上色。角色传 ACCENT，和整树换肤按无障碍描述（没有「按键 」前缀）推出来的一致，不让键帽缓存来回失效。
        s.imeStyler.styleButton(select, true);
        if (trackpadSelect != null) {
            trackpadSelect.setText(selecting ? "取消选择" : "选择");
            trackpadSelect.setContentDescription(selecting ? "取消选择" : "选择模式");
            ViewPolicy.setSelected(trackpadSelect, selecting);
            trackpadSelect.setActivated(selecting);
            s.imeStyler.styleButton(trackpadSelect, true);
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
        } else if (action == TextEditPanelModel.Action.SELECT_LINE) {
            selectLine(target);
            selecting = false;
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
            } else if (!isCursorMove(action) || cursorSteps(target, action, 1, selecting) > 0) {
                sendKey(target, TextEditPanelModel.keyCode(action),
                    TextEditPanelModel.metaState(action, selecting));
            }
            if (TextEditPanelModel.endsSelecting(action)) selecting = false;
        }
        renderState();
    }

    private void selectLine(InputConnection target) {
        try {
            android.view.inputmethod.ExtractedTextRequest request =
                new android.view.inputmethod.ExtractedTextRequest();
            android.view.inputmethod.ExtractedText extracted = target.getExtractedText(request, 0);
            if (extracted == null || extracted.text == null || extracted.startOffset != 0
                    || extracted.partialStartOffset != -1) {
                s.notice("当前输入框不支持选择行");
                return;
            }
            TextEditLineSelection.Range range = TextEditLineSelection.range(
                extracted.text, extracted.selectionStart, extracted.selectionEnd);
            if (range == null || !target.setSelection(range.start(), range.end()))
                s.notice("当前输入框不支持选择行");
        } catch (RuntimeException ignored) {
            s.notice("当前输入框不支持选择行");
        }
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
        for (TextEditPanelModel.Action action : new TextEditPanelModel.Action[] {
                TextEditPanelModel.Action.LEFT, TextEditPanelModel.Action.UP, TextEditPanelModel.Action.SELECT,
                TextEditPanelModel.Action.DOWN, TextEditPanelModel.Action.RIGHT})
            ViewPolicy.setVisible(buttons.get(action), !enabled);
        ViewPolicy.setVisible(trackpadSelect, enabled);
        modeButton.setText(enabled ? "切换为方向键" : "切换为触控板");
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

        void retarget(Object previous, Object next) { hold.retarget(previous, next); }

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
        boolean extend = selecting || steps.selecting();
        // 越过文档首尾的步数在这里扣掉（#10141），被扣掉的步不发键也不震动，手势照常继续。
        TextCursorEdgePolicy.Plan plan = cursorPlan(target, steps.horizontal(), steps.vertical(), extend);
        TextEditPanelModel.Action horizontal = plan.horizontal() < 0 ? TextEditPanelModel.Action.LEFT : TextEditPanelModel.Action.RIGHT;
        TextEditPanelModel.Action vertical = plan.vertical() < 0 ? TextEditPanelModel.Action.UP : TextEditPanelModel.Action.DOWN;
        for (TextEditPanelModel.Action action : new TextEditPanelModel.Action[] {horizontal, vertical}) {
            int count = action == horizontal ? Math.abs(plan.horizontal()) : Math.abs(plan.vertical());
            for (int index = 0; index < count; index++) {
                if (s.connection != target || !sendKey(target, TextEditPanelModel.keyCode(action),
                        TextEditPanelModel.metaState(action, extend))) return false;
                if (trackpadSettings != null && trackpadSettings.haptics() && s.hapticsEnabled)
                    trackpad.performHapticFeedback(android.view.HapticFeedbackConstants.KEYBOARD_TAP);
            }
        }
        refreshPreview();
        return true;
    }

    /**
     * 这一批方向键里实际发几个（{@link TextCursorEdgePolicy}）：光标已在文档首尾时往外的键不发，免得编辑器把它当作焦点导航、把焦点交给相邻的控件。回读在主线程同步进行，每批只读一次、只读要走的那一侧；密码框不读。空格拖动也经这里。
     */
    TextCursorEdgePolicy.Plan cursorPlan(InputConnection target, int horizontal, int vertical, boolean extend) {
        return TextCursorEdgePolicy.plan(horizontal, vertical, extend, selectionStart, selectionEnd,
            s.editorInputType, new TextCursorEdgePolicy.Editor() {
                @Override public CharSequence before(int limit) { return InputConnectionGuard.textBeforeCursor(target, limit); }
                @Override public CharSequence after(int limit) { return InputConnectionGuard.textAfterCursor(target, limit); }
                @Override public CharSequence selected() { return InputConnectionGuard.selectedText(target); }
                @Override public boolean confirmsEmpty() { return extractedTextEmpty(target); }
            });
    }

    /** 编辑器经 getExtractedText 报的全文是不是空的。不维护文字、只收按键的编辑器（BaseInputConnection 的默认实现）不提供抽取文字，确认不了；调用失败同样算确认不了。 */
    private static boolean extractedTextEmpty(InputConnection target) {
        try {
            android.view.inputmethod.ExtractedTextRequest request =
                new android.view.inputmethod.ExtractedTextRequest();
            request.hintMaxChars = 1;
            android.view.inputmethod.ExtractedText extracted = target.getExtractedText(request, 0);
            return extracted != null && extracted.text != null && extracted.text.length() == 0
                && extracted.startOffset == 0 && extracted.partialStartOffset == -1;
        } catch (RuntimeException disconnected) {
            return false;
        }
    }

    private int cursorSteps(InputConnection target, TextEditPanelModel.Action action, int count, boolean extend) {
        int horizontal = action == TextEditPanelModel.Action.LEFT ? -count : action == TextEditPanelModel.Action.RIGHT ? count : 0;
        int vertical = action == TextEditPanelModel.Action.UP ? -count : action == TextEditPanelModel.Action.DOWN ? count : 0;
        TextCursorEdgePolicy.Plan plan = cursorPlan(target, horizontal, vertical, extend);
        return Math.abs(plan.horizontal()) + Math.abs(plan.vertical());
    }

    void editorChanged(int start, int end) {
        close();
        selectionChanged(start, end);
    }

    /**
     * 同一输入框重启连接（#10141）：面板、选择模式和进行中的手势都留着，改认新连接；只作废旧连接上在途的选词回读和自身选区记录，放大镜重新绑到新连接。WebView 一类编辑器在拖动中途也会重启连接，原来一律按换输入框处理，面板随之关掉。
     */
    void editorRestarted(InputConnection previous, int start, int end) {
        InputConnection next = s.connection;
        selectionReads.cancel();
        selectionVersion.reset();
        if (trackpad != null) trackpad.retarget(previous, next);
        if (shortcutTouch != null) shortcutTouch.retarget(previous, next);
        if (previewActive) setPreviewActive(true);
        selectionChanged(start, end);
    }

    /** 编辑器最近一次报来的选区是不是这一对，两端都已知才算（同一输入框重启的佐证，见 MSIMEInputService.onStartInput）。 */
    boolean reportedSelection(int start, int end) {
        return start >= 0 && end >= 0 && selectionStart == start && selectionEnd == end;
    }

    void selectionChanged(int start, int end) {
        selectionVersion.reported(start, end);
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
        long version = selectionVersion.current();
        selectionReads.request(() -> readTrackpadSelection(target, tap), range -> {
            if (s.connection != target || version != selectionVersion.current() || !ViewPolicy.isVisible(s.textEditPanel)) return;
            if (range == null) {
                s.notice("当前编辑器未提供完整的词或行，仍可长按拖动选择");
                return;
            }
            s.selectionEcho.invalidate();
            try {
                if (target.setSelection(range.start(), range.end())) selectionVersion.applied(range.start(), range.end());
                else s.notice("当前输入框不支持选择");
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
