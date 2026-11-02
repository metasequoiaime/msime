package app.msime.android;

import app.msime.android.LayoutPolicy;


import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.Button;
import app.msime.android.KeyboardGeometry;
import android.widget.LinearLayout;

/**
 * 键盘底行：按 {@link KeyboardActionRow#designEntries} 排布。26 键为 123 | 符 | 空格 | ， | 中 | ↵，逗号长按输入句号；14 键和大千注音保留独立逗号句号；手写为 123 | ， | 空格 | 。 | 中 | 重写 | ⌫ | ↵，逗号长按输入问号、句号长按输入叹号；拼音九键和笔画为 符 | 123 | 空格 | 中 | ↵。26 键、14/18 键和九键的字母层底行另按共享偏好里的底行样式排（{@link TouchKeyStyle}，{@link MSIMEInputService#bottomRowStyle}）。地球键按系统能力插在 123 之后，空格保留语音与移光标手势，回车始终使用操作色。
 *
 * <p>空格与回车换成画图标的 {@link SpaceKeyFace} / {@link KeyboardIconKey}：SVC 建的原按钮留作点击的执行者（它带着计数、反馈与动作），新键点一下就让原按钮 performClick，所以 SVC 里的动作不必搬动；节点 text 照旧由 SVC 设置（「空格」、「换行/确认/…」）。
 */
final class ImeBottomRow {
    private final MSIMEInputService s;
    /** 底行的句号键（新设计里逗号右侧那个）；第一次排布时建。手写底行上它带「！」角标，长按输入叹号。 */
    private KeyHintButton periodButton;
    /** 手写底行的「重写」和 ⌫：书写区铺满整行后从它右侧的列搬到这里；第一次排布手写底行时建。 */
    private Button rewriteButton;
    private Button handwritingDeleteButton;
    /** 九键数字层的唯一 0，普通层不挂载。 */
    private Button zeroButton;
    /** 表情键样式底行的 😀；第一次排布这种底行时建，之后和其他常驻键一样换行时挪过去。 */
    private Button emojiButton;
    /** 分离式键盘右半边内侧的第二个空格键；第一次分离时建，换了一套控件（onCreateInputView）时作废重建。 */
    private SpaceKeyFace splitSpace;
    private boolean bottomCommaSwipeBound;
    private Button bottomCommaSwipeButton;

    ImeBottomRow(MSIMEInputService s) {
        this.s = s;
    }

    /** 长空格样式（26 键与 14/18 键）的逗号上滑或下滑输入句号，普通短按和已有长按动作保持原样。 */
    void updateCommaSwipe() {
        if (s.quickPunctuationButton == null) return;
        boolean enabled = s.commaSwipesPeriod(s.imeLetterRows.drawnTouchLayout());
        if (enabled == bottomCommaSwipeBound
                && bottomCommaSwipeButton == s.quickPunctuationButton) return;
        if (bottomCommaSwipeButton != null) bottomCommaSwipeButton.setOnTouchListener(null);
        bottomCommaSwipeButton = s.quickPunctuationButton;
        bottomCommaSwipeBound = enabled;
        if (enabled) bindCommaSwipe(bottomCommaSwipeButton);
    }

    /** 数字层的逗号键和字母层共用同一手势。上滑和下滑都输入句号：#9881 要的是下滑，而字母键滑动输入符号默认是上滑（`platform.android.swipe_symbols_direction`），只认下滑时照字母键习惯上滑什么也不出。逗号键没有别的纵向手势，两个方向都接不会冲突。 */
    void bindCommaSwipe(Button button) {
        final float[] start = new float[2];
        final boolean[] swiped = new boolean[1];
        // 门槛与字母键滑动输入符号相同（14 dp，严格大于），不用约 8 dp 的 touch slop：上滑也算之后，点逗号时手指往上带一下就会误出句号。
        final float threshold = SwipeHintPolicy.THRESHOLD_DP * s.getResources().getDisplayMetrics().density;
        final int longPress = ViewConfiguration.getLongPressTimeout();
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    start[0] = event.getX();
                    start[1] = event.getY();
                    swiped[0] = false;
                    return false;
                }
                case MotionEvent.ACTION_MOVE -> {
                    // 按满长按时限还没滑出去，这次触摸已经归长按（长按逗号输入句号），之后手指挪动不能再起滑动，否则松手会多输入一个。
                    if (!swiped[0] && event.getEventTime() - event.getDownTime() >= longPress) return false;
                    float dx = event.getX() - start[0];
                    float dy = event.getY() - start[1];
                    if (Math.abs(dy) > threshold && Math.abs(dy) > Math.abs(dx)) {
                        swiped[0] = true;
                        button.cancelLongPress();
                        button.setPressed(false);
                    }
                    return swiped[0];
                }
                case MotionEvent.ACTION_UP -> {
                    if (!swiped[0]) return false;
                    s.imeKeyFeedback.playFeedback(button);
                    s.countKey(button);
                    s.type('.');
                    swiped[0] = false;
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    swiped[0] = false;
                    return false;
                }
                default -> { return swiped[0]; }
            }
        });
    }

    /** 回车键始终是 accent 填充（设计：空闲画 ↵ 图标，组词时显示「确认」）。 */
    KeyboardKeyRole returnKeyRole() {
        return KeyboardKeyRole.RETURN;
    }

    /** 偏好里的一个布尔触控开关；没有偏好快照或没有这个键时按 `fallback`。 */
    boolean touchPreference(String key) {
        return s.localSettings.bool(key);
    }

    /** 把 SVC 建的空格、回车换成画图标的键（每次 onCreateInputView 新建控件后各换一次）。 */
    void ensureFaces() {
        if (s.spaceButton != null && !(s.spaceButton instanceof SpaceKeyFace)) {
            Button original = s.spaceButton;
            SpaceKeyFace face = forwardFace(original, new SpaceKeyFace(s), "Space");
            s.imeStyler.styleButton(face, false);
            bindSpaceCursor(face);
            s.spaceButton = face;
            splitSpace = null;
        }
        if (s.enterButton != null && !(s.enterButton instanceof KeyboardIconKey)) {
            Button original = s.enterButton;
            KeyboardIconKey key = forwardFace(original,
                new KeyboardIconKey(s, s.clipboardMode == ClipboardModePolicy.Mode.VISITOR
                    ? KeyboardIconKey.Kind.TRASH : KeyboardIconKey.Kind.RETURN), "Enter");
            // 组词时 SVC 把 text 设成「确认」（日语「確定」），这时按文字画；其他动作文字一律画 ↵。
            key.setTextFaces(java.util.Set.of("确认", "確定"));
            KeyboardTypographyPolicy.setKeyTextSize(key, 15);
            ViewPolicy.setTypefaceStyle(key, android.graphics.Typeface.BOLD);
            key.setKeyboardRole(KeyboardKeyRole.RETURN);
            s.imeStyler.styleButton(key, KeyboardKeyRole.RETURN, s.skin);
            bindReturnClipboardMode(key);
            s.bindReturnTranslationHold(key);
            s.enterButton = key;
        }
    }

    /** 回车键长按后向上滑动切换本机/访客剪贴板；无效手势仍保持原来的确认动作。 */
    private void bindReturnClipboardMode(KeyboardIconKey button) {
        final float[] start = new float[2];
        final boolean[] moved = new boolean[1];
        final boolean[] longClicked = new boolean[1];
        final Runnable[] longPress = new Runnable[1];
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    start[0] = event.getRawX();
                    start[1] = event.getRawY();
                    moved[0] = false;
                    longClicked[0] = false;
                    longPress[0] = () -> {
                        longPress[0] = null;
                        if (!moved[0]) {
                            longClicked[0] = button.performLongClick();
                            button.setPressed(false);
                        }
                    };
                    s.main.postDelayed(longPress[0], ViewConfiguration.getLongPressTimeout());
                    button.setPressed(true);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    float dx = event.getRawX() - start[0];
                    float dy = event.getRawY() - start[1];
                    if (Math.hypot(dx, dy) > ViewConfiguration.get(s).getScaledTouchSlop()) {
                        moved[0] = true;
                        if (longPress[0] != null) s.main.removeCallbacks(longPress[0]);
                        longPress[0] = null;
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    if (longPress[0] != null) s.main.removeCallbacks(longPress[0]);
                    longPress[0] = null;
                    int[] location = new int[2];
                    if (s.keyboardRoot != null) s.keyboardRoot.getLocationOnScreen(location);
                    float rootX = event.getRawX() - location[0];
                    float rootY = event.getRawY() - location[1];
                    float startRootX = start[0] - location[0];
                    float startRootY = start[1] - location[1];
                    boolean toggle = ClipboardModePolicy.isValidSwipe(startRootX, startRootY,
                        rootX, rootY, button.getWidth(), button.getHeight(),
                        s.keyboardRoot == null ? 0 : s.keyboardRoot.getHeight());
                    button.setPressed(false);
                    if (longClicked[0]) return true;
                    if (toggle) s.toggleClipboardMode(button);
                    else button.performClick();
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    if (longPress[0] != null) s.main.removeCallbacks(longPress[0]);
                    longPress[0] = null;
                    button.setPressed(false);
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    private <T extends Button> T forwardFace(Button original, T face, String keyId) {
        face.setText(original.getText());
        face.setContentDescription(original.getContentDescription());
        ViewPolicy.bindClick(face, original::performClick);
        s.keyId(face, keyId);
        s.imeKeyFeedback.stageFace(original, face);
        return face;
    }

    /** 空格键面上画不画麦克风：跟着「长按空格语音输入」走。关掉后长按空格不进语音（{@link #bindSpaceCursor} 按同一个开关武装长按），键面也不再暗示能按住说话（#10143）。组字中长按同样不进语音，但这里不看它，否则每打一个码麦克风就闪一下。 */
    private boolean spaceShowsMic() {
        return touchPreference(AndroidLocalSettings.SPACE_VOICE);
    }

    /** 空格键面上的方案短名。 */
    private String spaceLabel() {
        return SpaceKeyFace.schemeLabel(s.selectedScheme, s.wubiProfile, s.dedicatedEnglish);
    }

    void resetSpaceCursor() {
        s.cursorMovement.cancel();
        if (s.spaceButton != null) {
            s.spaceButton.setPressed(false);
            s.spaceButton.setText(s.spaceKeyTitle());
            s.spaceButton.setContentDescription(s.spaceKeyDescription());
            if (s.spaceButton instanceof SpaceKeyFace face) face.setTransientLabel("");
        }
        if (splitSpace != null) {
            splitSpace.setPressed(false);
            splitSpace.setTransientLabel("");
        }
        syncSplitSpace();
        if (s.japaneseSpaceKey != null && s.japaneseSpaceKey != s.spaceButton) {
            s.japaneseSpaceKey.setPressed(false);
            s.japaneseSpaceKey.setText(s.spaceKeyTitle());
            s.japaneseSpaceKey.setContentDescription(s.spaceKeyDescription());
        }
    }

    /**
     * 分离式键盘右半边的空格键。它不经过原来的空格按钮转发，而是自己绑定同一个 {@link MSIMEInputService#space} 动作：按键反馈、动画和按键计数落在用户真正按下的那个键上；键位 id 同样是 `Space`，按键音按空格键分类，热力图也计入空格。长按语音和拖动移光标用同一套 {@link #bindSpaceCursor}。
     */
    Button splitSpaceButton() {
        if (splitSpace == null) {
            SpaceKeyFace face = new SpaceKeyFace(s);
            s.keyId(face, "Space");
            s.bindCountedAction(face, s::space);
            bindSpaceCursor(face);
            splitSpace = face;
        }
        splitSpace.setKeyboardRole(KeyboardKeyRole.KEY);
        s.imeStyler.styleButton(splitSpace, KeyboardKeyRole.KEY, s.skin);
        syncSplitSpace();
        return splitSpace;
    }

    /** 让右半边的空格键与主空格键显示同样的文字、描述、方案短名和可用状态；它自己正在拖动光标时不改文字。 */
    void syncSplitSpace() {
        if (splitSpace == null || s.spaceButton == null) return;
        if (!s.cursorMovement.isActive()) {
            splitSpace.setText(s.spaceKeyTitle());
            splitSpace.setContentDescription(s.spaceKeyDescription());
        }
        splitSpace.setShowsMic(spaceShowsMic());
        splitSpace.setSchemeLabel(spaceLabel());
        ViewPolicy.setEnabled(splitSpace, s.spaceButton.isEnabled());
    }

    void moveEditorCursor(int offset) {
        if (offset == 0 || s.connection == null) return;
        // 方向键由编辑器自己解释（换行、代理对、双向文字），落点算不出来。
        s.selectionEcho.invalidate();
        // 光标已在文档首尾时不再往外发（#10141），否则编辑器把多出来的方向键当作焦点导航。
        int steps = s.imeTextEditPanel.cursorPlan(s.connection, offset, 0, false).horizontal();
        int keyCode = steps < 0 ? KeyEvent.KEYCODE_DPAD_LEFT : KeyEvent.KEYCODE_DPAD_RIGHT;
        for (int index = 0; index < Math.abs(steps); index++) s.sendEditorKeyEvents(keyCode);
    }

    /** 一个空格键自己的手势状态。分离式键盘有两个空格键，两根拇指可能先后按住它们，状态不能共用，否则后按下的会把先按下的那次长按计时和手势仲裁一起冲掉。 */
    private final class SpacePress {
        final SpaceGesturePolicy gesture = new SpaceGesturePolicy();
        android.view.inputmethod.InputConnection copyTarget;
        /** 长按（450 ms）打开语音输入的计时；拖动先越过阈值时这次长按作废。 */
        Runnable longPress;

        void cancelLongPress() {
            if (longPress != null) s.main.removeCallbacks(longPress);
            longPress = null;
        }
    }

    /**
     * 空格键手势：{@link SpaceGesturePolicy} 仲裁长按语音、水平移光标和上滑复制全文，先满足的一方胜出；下滑作废，短按输入普通空格。
     *
     * <p>长按在键区里开始的语音是按住说话：松手结束录音并出结果，按住期间停顿不自动结束；手指按当前面板的实际按钮范围选择取消、锁定或发送，触摸被系统取消时丢弃录音。
     */
    void bindSpaceCursor(Button button) {
        final float[] origin = new float[2];
        final boolean[] dragging = new boolean[1];
        final boolean[] cancelled = new boolean[1];
        final boolean[] voiced = new boolean[1];
        final boolean[] copyAvailable = new boolean[1];
        // 这次按压开始的语音是按住说话（键区里真的开始了一次新的聆听）。
        final boolean[] holding = new boolean[1];
        final long[] voiceBefore = new long[1];
        final int touchSlop = ViewConfiguration.get(s).getScaledTouchSlop();
        final SpacePress press = new SpacePress();
        final Runnable startVoice = () -> {
            if (voiced[0]) return;
            voiced[0] = true;
            cancelled[0] = true;
            button.setPressed(false);
            s.imeKeyFeedback.playFeedback(button);
            voiceBefore[0] = s.imeVoiceEntry.generation();
            s.startVoiceRecognition();
            s.imeVoiceEntry.plainIndicator(voiceBefore[0]);
        };
        // 手指还按着时开始的语音改成按住说话；松手那一刻才补判出的长按仍按点按开始，否则刚开始就被这次松手结束。
        final Runnable holdVoice = () -> {
            if (voiced[0]) return;
            s.imeVoiceEntry.startHeld(startVoice);
            holding[0] = s.imeVoiceEntry.hold(voiceBefore[0]);
        };
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    dragging[0] = false;
                    cancelled[0] = false;
                    voiced[0] = false;
                    holding[0] = false;
                    button.setPressed(true);
                    button.getParent().requestDisallowInterceptTouchEvent(true);
                    // 组字中或关了长按语音时不武装长按：这次按压保持普通空格与拖动移光标，与 develop 一致；否则慢一点的空格会被吞掉、停顿后的拖动也移不了光标。
                    boolean voice = touchPreference(AndroidLocalSettings.SPACE_VOICE) && s.voiceInsertionReady();
                    copyAvailable[0] = s.imeTextEditPanel.canSwipeCopyAll();
                    press.copyTarget = s.connection;
                    press.gesture.down(SystemClock.uptimeMillis(),
                        DimensionPolicy.fromPixels(s, event.getX()), DimensionPolicy.fromPixels(s, event.getY()),
                        voice, copyAvailable[0]);
                    press.cancelLongPress();
                    if (voice) {
                        press.longPress = () -> {
                            press.longPress = null;
                            press.gesture.tick(SystemClock.uptimeMillis());
                            if (press.gesture.state() == SpaceGesturePolicy.State.VOICE
                                    && s.voiceInsertionReady()) holdVoice.run();
                        };
                        s.main.postDelayed(press.longPress, SpaceGesturePolicy.LONG_PRESS_MS);
                    }
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (voiced[0]) {
                        if (holding[0]) {
                            s.imeVoiceEntry.moveHold(event.getRawX(), event.getRawY());
                        }
                        return true;
                    }
                    if (press.gesture.state() == SpaceGesturePolicy.State.COPY) {
                        press.gesture.move(SystemClock.uptimeMillis(), DimensionPolicy.fromPixels(s, event.getX()),
                            DimensionPolicy.fromPixels(s, event.getY()));
                        showSpaceCopyLabel(button, press.gesture.copyArmed());
                        return true;
                    }
                    if (!dragging[0] && !cancelled[0]) {
                        float horizontal = event.getX() - origin[0];
                        float vertical = event.getY() - origin[1];
                        press.gesture.move(SystemClock.uptimeMillis(),
                            DimensionPolicy.fromPixels(s, event.getX()), DimensionPolicy.fromPixels(s, event.getY()));
                        SpaceGesturePolicy.State state = press.gesture.state();
                        if (state == SpaceGesturePolicy.State.COPY) {
                            press.cancelLongPress();
                            button.setPressed(false);
                            s.imeKeyFeedback.playFeedback(button);
                            showSpaceCopyLabel(button, true);
                            return true;
                        }
                        if (state == SpaceGesturePolicy.State.VOICE) {
                            // 只有按下时启用了长按语音，状态机才可能进入语音。
                            press.cancelLongPress();
                            if (s.voiceInsertionReady()) holdVoice.run();
                            return true;
                        }
                        if (state == SpaceGesturePolicy.State.PRESSED) {
                            boolean approachingCopy = copyAvailable[0] && vertical < 0
                                && -vertical < s.pixels((int) SpaceGesturePolicy.COPY_THRESHOLD_DP);
                            if (!approachingCopy && Math.abs(vertical) > touchSlop
                                    && Math.abs(vertical) >= Math.abs(horizontal)) {
                                press.cancelLongPress();
                                press.gesture.cancel();
                                cancelled[0] = true;
                                button.setPressed(false);
                            }
                            return true;
                        }
                        // 拖动先越过阈值：这次按压不再算长按。
                        press.cancelLongPress();
                        if (s.connection == null || !touchPreference(AndroidLocalSettings.SPACE_CURSOR)) {
                            cancelled[0] = true;
                            button.setPressed(false);
                            return true;
                        }
                        s.command(2);
                        s.cursorMovement.begin(origin[0], s.connection);
                        dragging[0] = s.cursorMovement.isActive();
                        cancelled[0] = !dragging[0];
                        button.setPressed(false);
                        if (dragging[0]) {
                            button.setText("移动光标");
                            if (button instanceof SpaceKeyFace face) face.setTransientLabel("移动光标");
                            button.setContentDescription("正在移动光标");
                            moveEditorCursor(s.cursorMovement.advance(
                                event.getX(), s.connection, s.pixels(12)));
                        }
                        return true;
                    }
                    if (dragging[0]) {
                        moveEditorCursor(s.cursorMovement.advance(
                            event.getX(), s.connection, s.pixels(12)));
                        if (!s.cursorMovement.isActive()) {
                            dragging[0] = false;
                            cancelled[0] = true;
                            resetSpaceCursor();
                        }
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    press.cancelLongPress();
                    var copyTarget = press.copyTarget;
                    press.copyTarget = null;
                    boolean copyGesture = press.gesture.state() == SpaceGesturePolicy.State.COPY;
                    SpaceGesturePolicy.Outcome outcome = press.gesture.up(SystemClock.uptimeMillis());
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    button.setPressed(false);
                    if (copyGesture) {
                        resetSpaceCursor();
                        s.countKey(button);
                        if (outcome == SpaceGesturePolicy.Outcome.COPY && copyTarget == s.connection)
                            s.imeTextEditPanel.copyAllFromGesture();
                        return true;
                    }
                    if (!voiced[0] && !dragging[0] && !cancelled[0]
                            && outcome == SpaceGesturePolicy.Outcome.VOICE
                            && touchPreference(AndroidLocalSettings.SPACE_VOICE)
                            && s.voiceInsertionReady()) {
                        // 计时回调还没来得及跑就松手了：仍按长按处理。
                        startVoice.run();
                    }
                    if (voiced[0]) {
                        s.countKey(button);
                        if (holding[0]) {
                            holding[0] = false;
                            s.imeVoiceEntry.releaseHold(event.getRawX(), event.getRawY(), false);
                        }
                        return true;
                    }
                    if (dragging[0] || cancelled[0]) {
                        // The thumb still pressed the space bar; dragging it moved the cursor instead of typing.
                        s.countKey(button);
                        resetSpaceCursor();
                    } else button.performClick();
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    press.cancelLongPress();
                    press.copyTarget = null;
                    press.gesture.cancel();
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    dragging[0] = false;
                    cancelled[0] = true;
                    resetSpaceCursor();
                    if (holding[0]) {
                        holding[0] = false;
                        s.imeVoiceEntry.releaseHold(0f, 0f, true);
                    }
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    private void showSpaceCopyLabel(Button button, boolean armed) {
        String label = armed ? "松开复制全文" : "取消复制";
        button.setText(label);
        button.setContentDescription(label);
        if (button instanceof SpaceKeyFace face) face.setTransientLabel(label);
    }

    Button actionRowKey(KeyboardActionRow.DesignSlot slot) {
        return switch (slot) {
            case SYMBOL_PANEL -> s.symbolPanelButton;
            case LAYER -> s.layerButton;
            case LANGUAGE -> s.languageButton;
            case GLOBE -> s.globeButton;
            case EMOJI -> emojiButton();
            case COMMA -> s.quickPunctuationButton;
            case SPACE -> s.spaceButton;
            case ZERO -> zeroButton();
            case PERIOD -> periodButton();
            case REWRITE -> rewriteButton();
            case DELETE -> handwritingDeleteButton();
            case RETURN -> s.enterButton;
        };
    }

    private Button zeroButton() {
        if (zeroButton == null) {
            zeroButton = s.keyId(s.nineKeyGridKey("0", "数字 0",
                () -> s.imeLayoutRows.commitNumberPadText("0")), "Nine0");
            KeyboardTypographyPolicy.setFittedKeyTextSize(zeroButton, 14, 22);
        }
        return zeroButton;
    }

    /** 表情键样式底行的 😀：点按打开表情面板，记作 `SoftEmoji`，与工具栏的表情按钮同一个键位；符号面板仍可长按 123 打开。 */
    private Button emojiButton() {
        if (emojiButton == null) {
            KeyboardIconKey icon = new KeyboardIconKey(s, KeyboardIconKey.Kind.EMOJI);
            icon.setText("😀");
            icon.setContentDescription(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.EMOJI, true));
            s.bindCountedAction(icon, () -> {
                s.imePanels.showEmojiPicker();
                // 与 123 层底行的表情键相同：刷新工具栏的返回动作，隐藏工具栏时面板要临时显示它。
                s.render();
            });
            emojiButton = s.keyId(icon, "SoftEmoji");
        }
        return emojiButton;
    }

    /** 底行句号键：中文标点模式画「。」，否则「.」；发出的都是 `.`，由 Engine 按标点模式转换。手写底行上长按输入叹号（{@link KeyboardActionRow#usesHandwritingMarks}），其他底行长按不做事。 */
    private Button periodButton() {
        if (periodButton == null) {
            KeyHintButton period = new KeyHintButton(s);
            KeyboardTypographyPolicy.normalizeKeyCap(period);
            ViewPolicy.setAllCapsFalse(period);
            period.setText(".");
            s.imeStyler.styleButton(period, false);
            s.bindCountedAction(period, () -> s.type('.'));
            period.setOnLongClickListener(ignored -> {
                if (!s.handwritingMarksShown()) return false;
                s.imeKeyFeedback.playFeedback(period);
                s.countKey(period);
                s.type('!');
                return true;
            });
            periodButton = s.keyId(period, "Period");
            KeyboardTypographyPolicy.setKeyTextSize(periodButton, 22);
        }
        return periodButton;
    }

    /** 手写底行的「重写」：清空书写区和还没停笔识别的笔迹。 */
    private Button rewriteButton() {
        if (rewriteButton == null) {
            rewriteButton = s.keyboardKey("重写", "清空手写", () -> {
                s.imeLayoutRows.cancelPendingInk();
                s.clearHandwriting();
            });
        }
        return rewriteButton;
    }

    /** 手写底行的 ⌫：有笔迹时撤回一笔，没有时删除光标前的字符；和其他布局的删除键一样按住连删、上滑快速删除（#5585）。 */
    private Button handwritingDeleteButton() {
        if (handwritingDeleteButton == null) {
            handwritingDeleteButton = s.keyId(s.backspaceKey(s::deleteFromHandwriting), "Backspace");
            s.imeLetterRows.bindBackspaceRepeat(handwritingDeleteButton, s::deleteFromHandwriting);
        }
        return handwritingDeleteButton;
    }

    /**
     * 把底行的一个常驻键（{@link #actionRowKey}）挂进 {@code parent}：先从原来的父视图摘下，再按功能行的规矩给角色、键帽、样式和字号。功能行自己用它，右侧大回车外框（{@link ImeLayoutRows#rebuildNineKeyRows}）把 符、123、空格、中/英、回车挂进外框时也用它，两处画出来的键一样。
     */
    void placeKey(LinearLayout parent, KeyboardActionRow.DesignSlot slot, Button key, LinearLayout.LayoutParams params) {
        if (key.getParent() instanceof android.view.ViewGroup previous) previous.removeView(key);
        KeyboardKeyRole role = switch (slot) {
            case RETURN -> returnKeyRole();
            case SPACE, ZERO, COMMA, PERIOD -> KeyboardKeyRole.KEY;
            default -> KeyboardKeyRole.ACCENT;
        };
        if (key instanceof KeyboardPressButton press) press.setKeyboardRole(role);
        // 符、123、中/英、逗号由通用的 button() 建出来，没有经过键帽工厂；放进底行时才知道它们是键帽。
        KeyboardTypographyPolicy.normalizeKeyCap(key);
        s.imeStyler.styleButton(key, role, s.skin);
        if (slot == KeyboardActionRow.DesignSlot.COMMA)
            KeyboardTypographyPolicy.setKeyTextSize(key, 22);
        else if (role == KeyboardKeyRole.ACCENT)
            KeyboardTypographyPolicy.setKeyTextSize(key, 15);
        ViewPolicy.show(key);
        parent.addView(key, params);
    }

    /** #+= 扩展符号页自带底行，这时功能行整行不显示；数字层的另一页是数字九宫格，用功能行排它的底行。注音的 #+= 是共享符号行，同样用功能行（{@link ImeLetterRows#sharedSymbolRowsDrawn}）。 */
    boolean layerOwnsBottomRow() {
        return s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS && !s.imeLetterRows.numberPadDrawn()
            && !s.imeLetterRows.sharedSymbolRowsDrawn();
    }

    /** 句号键的键面、角标和可用状态跟着标点模式与键面走；句号键还没建时不做事。 */
    private void refreshPeriodFace() {
        if (periodButton == null) return;
        boolean chinesePunctuation = s.sendsChinesePunctuation();
        String periodFace = KeyboardActionRow.punctuationFace(
            KeyboardActionRow.DesignSlot.PERIOD, chinesePunctuation);
        boolean exclamation = s.handwritingMarksShown();
        // 句号沿用字体里的大小和基线位置，避免放大居中后看起来像圆圈。
        periodButton.setText(periodFace);
        periodButton.setCornerHint(exclamation
            ? ChineseSymbolFaces.face("!", chinesePunctuation, s.punctuationMarks()) : "");
        periodButton.setContentDescription("按键 " + periodFace + (exclamation ? "；长按输入叹号" : ""));
        // type('.') 没有会话时也会直接上屏字面句点（密码框等），与旁边的逗号键一致。
        ViewPolicy.setEnabled(periodButton, s.connection != null);
    }

    /** 按当前键面重挂常驻按键，保留空格手势与删除连按的状态和监听器。 */
    void updateActionRow() {
        if (s.actionRow == null) return;
        ensureFaces();
        s.imeStyler.refreshSeasonIfNeeded();
        s.imeLetterRows.updateSecondRowIndent();
        // 注音的共享符号行（#+= 页）按大千注音排底行（123 |（地球）| ， | 空格 | 。 | 中/英 | 回车）：注音九键的字母层把这几个键挪进了自己的网格，没有底行可借。这时不是数字九宫格，layout 只用来排这一行和记签名。
        int layout = s.imeLetterRows.sharedSymbolRowsDrawn() ? KeyboardLayout.ZHUYIN_LAYOUT
            : s.imeLetterRows.drawnTouchLayout();
        boolean numberPad = s.imeLetterRows.numberPadDrawn();
        boolean globe = s.offersGlobeKey();
        boolean ownBottom = layerOwnsBottomRow();
        refreshPeriodFace();
        if (s.spaceButton instanceof SpaceKeyFace face) {
            face.setShowsMic(spaceShowsMic());
            // 数字页的空格主标画 ␣，麦克风只在右上角提示长按可说话（#10059）；方案短名只在字母层显示。
            face.setNumberPadFace(numberPad);
            face.setSchemeLabel(numberPad ? "" : spaceLabel());
        }
        // 借用数字页的 0 记普通数字键，原拼音九键才记 Nine0；同一个常驻按键每次按当前键面重登。
        if (numberPad) s.imeLayoutRows.bindNumberDigit(zeroButton(), 9);
        syncSplitSpace();
        if (s.globeButton != null) s.globeButton.setContentDescription("切换输入法");
        // Every keystroke reaches render(), and re-parenting eight keys under the pressed one is a
        // relayout the user can see. The row only changes when the surface does.
        // 分离时空格拆成两半、空隙按可见键居中，所以常用标点键显示与否也会改变排布。
        boolean split = !ownBottom && s.splitKeyboardDrawn();
        boolean swapLanguageNumber = s.localSettings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS);
        TouchKeyStyle.Row style = s.bottomRowStyle(layout);
        // 英文 26 键在空格左边多一个句点，中英切换时底行要重排。
        boolean english = s.dedicatedEnglish;
        // 「符」与「123」互换是键位习惯，密码框和数字字段里也照样互换（样式在那里退回标准底行，标准底行同样有 符）。
        boolean symbolFirst = s.twentySixSymbolFirst;
        TouchKeyStyle.NumberPadSpace numberPadSpace = s.numberPadSpace;
        // 中/英在空格左边同样是位置习惯，密码框和数字字段里照用户的选择，不退回。
        boolean languageLeft = s.languageKeyLeft;
        String signature = layout + ":" + numberPad + ":" + globe + ":" + ownBottom + ":" + split + ":" + swapLanguageNumber
            + ":" + english + ":" + style + ":" + symbolFirst + ":" + numberPadSpace.resolved() + ":" + languageLeft
            + (split ? ":" + s.imeLetterRows.splitWidthDp() : "")
            + (split && s.quickPunctuationButton != null ? ":" + s.quickPunctuationButton.getVisibility() : "");
        // 数字九宫格的底行跟着进入它的键面排：返回键留在原来 123 那一格（签名里已有 layout，换入口会重排）。
        java.util.List<KeyboardActionRow.DesignEntry> entries = ownBottom ? java.util.List.of()
            : numberPad ? KeyboardActionRow.numberPadEntries(layout, globe, swapLanguageNumber, style, symbolFirst, numberPadSpace, languageLeft)
            : KeyboardActionRow.designEntries(layout, globe, swapLanguageNumber, false, english, style, symbolFirst, numberPadSpace, languageLeft);
        // 每次重申可见性：回复面板会藏起功能行，长按语音保留原行测量与触摸流并覆盖它。
        ViewPolicy.setVisible(s.actionRow, !entries.isEmpty());
        if (s.imeVoiceEntry != null && s.imeVoiceEntry.coversActionRow()) ViewPolicy.setInvisible(s.actionRow);
        // 中/英紧挨回车时，回车把键帽左边几 dp 让给中/英；中/英挪到空格左边后不再紧挨回车，回车不让。每次都重设：回车键换成图标键后是另一个视图。
        if (s.actionRow instanceof KeyboardKeyArea area) {
            boolean yield = KeyboardActionRow.languageBesideReturn(entries);
            area.setYield(yield ? s.enterButton : null, yield ? s.languageButton : null,
                DimensionPolicy.floatPixels(s, KeyboardActionRow.RETURN_YIELD_DP));
        }
        if (signature.equals(s.actionRowSignature)) {
            if (!ownBottom) s.updateQuickPunctuation();
            return;
        }
        s.actionRowSignature = signature;
        s.actionRow.removeAllViews();
        for (KeyboardActionRow.DesignEntry entry : entries) {
            Button key = actionRowKey(entry.slot());
            if (key == null) continue;
            placeKey(s.actionRow, entry.slot(), key, LayoutPolicy.weightedMatchParentParams(entry.weight()));
        }
        // 样式第一次把句号键排进底行时它刚建好，键面还是建键时的「.」，这里按当前标点模式补上。
        refreshPeriodFace();
        // The quick punctuation key hides itself when the scheme has no punctuation to offer, and
        // the loop above just told every slot it was visible.
        if (!ownBottom) s.updateQuickPunctuation();
        if (split) s.imeLetterRows.splitSpaceRow(s.actionRow);
        s.imeStyler.applyKeyboardGeometry();
    }
}
