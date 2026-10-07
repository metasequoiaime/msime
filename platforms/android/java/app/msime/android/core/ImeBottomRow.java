package app.msime.android;

import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.Button;
import app.msime.android.KeyboardGeometry;
import android.widget.LinearLayout;

/**
 * 键盘底行：按 {@link KeyboardActionRow#designEntries} 排布 123 | 中 | ， | 空格 | 。 | ↵（地球键只在系统允许切换输入法时插在中之后），空格键的长按语音与拖动移光标，回车键始终 accent 填充。
 *
 * <p>空格与回车换成画图标的 {@link SpaceKeyFace} / {@link KeyboardIconKey}：SVC 建的原按钮留作点击的执行者（它带着计数、反馈与动作），新键点一下就让原按钮 performClick，所以 SVC 里的动作不必搬动；节点 text 照旧由 SVC 设置（「空格」、「换行/确认/…」）。
 */
final class ImeBottomRow {
    private final MSIMEInputService s;
    /** 底行的句号键（新设计里逗号右侧那个）；第一次排布时建。 */
    private Button periodButton;
    /** 分离式键盘右半边内侧的第二个空格键；第一次分离时建，换了一套控件（onCreateInputView）时作废重建。 */
    private SpaceKeyFace splitSpace;

    ImeBottomRow(MSIMEInputService s) {
        this.s = s;
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
                new KeyboardIconKey(s, KeyboardIconKey.Kind.RETURN), "Enter");
            // 组词时 SVC 把 text 设成「确认」（日语「確定」），这时按文字画；其他动作文字一律画 ↵。
            key.setTextFaces(java.util.Set.of("确认", "確定"));
            KeyboardGeometry.setKeyTextSize(key, 15);
            key.setTypeface(android.graphics.Typeface.DEFAULT_BOLD);
            key.setKeyboardRole(KeyboardKeyRole.RETURN);
            s.imeStyler.styleButton(key, KeyboardKeyRole.RETURN, s.skin);
            s.enterButton = key;
        }
    }

    private <T extends Button> T forwardFace(Button original, T face, String keyId) {
        face.setText(original.getText());
        face.setContentDescription(original.getContentDescription());
        face.setOnClickListener(ignored -> original.performClick());
        s.keyId(face, keyId);
        s.imeKeyFeedback.stageFace(original, face);
        return face;
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
        splitSpace.setSchemeLabel(spaceLabel());
        ViewPolicy.setEnabled(splitSpace, s.spaceButton.isEnabled());
    }

    void moveEditorCursor(int offset) {
        // 方向键由编辑器自己解释（换行、代理对、双向文字），落点算不出来。
        if (offset != 0) s.selectionEcho.invalidate();
        int keyCode = offset < 0 ? KeyEvent.KEYCODE_DPAD_LEFT : KeyEvent.KEYCODE_DPAD_RIGHT;
        for (int index = 0; index < Math.abs(offset); index++) s.sendDownUpKeyEvents(keyCode);
    }

    /** 一个空格键自己的手势状态。分离式键盘有两个空格键，两根拇指可能先后按住它们，状态不能共用，否则后按下的会把先按下的那次长按计时和手势仲裁一起冲掉。 */
    private final class SpacePress {
        final SpaceGesturePolicy gesture = new SpaceGesturePolicy();
        /** 长按（450 ms）打开语音输入的计时；拖动先越过阈值时这次长按作废。 */
        Runnable longPress;

        void cancelLongPress() {
            if (longPress != null) s.main.removeCallbacks(longPress);
            longPress = null;
        }
    }

    /**
     * 空格键手势：{@link SpaceGesturePolicy} 仲裁长按 450 ms 打开语音（`touch_space_voice`）与水平拖动移动光标（`touch_space_cursor`），先满足的一方胜出；纵向滑走作废这次按压；两者都没发生时松手就是普通空格。
     */
    void bindSpaceCursor(Button button) {
        final float[] origin = new float[2];
        final boolean[] dragging = new boolean[1];
        final boolean[] cancelled = new boolean[1];
        final boolean[] voiced = new boolean[1];
        final int touchSlop = ViewConfiguration.get(s).getScaledTouchSlop();
        final SpacePress press = new SpacePress();
        final Runnable startVoice = () -> {
            if (voiced[0]) return;
            voiced[0] = true;
            cancelled[0] = true;
            button.setPressed(false);
            s.imeKeyFeedback.playFeedback(button);
            s.startVoiceRecognition();
        };
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    dragging[0] = false;
                    cancelled[0] = false;
                    voiced[0] = false;
                    button.setPressed(true);
                    button.getParent().requestDisallowInterceptTouchEvent(true);
                    // 组字中或关了长按语音时不武装长按：这次按压保持普通空格与拖动移光标，与 develop 一致；否则慢一点的空格会被吞掉、停顿后的拖动也移不了光标。
                    boolean voice = touchPreference(AndroidLocalSettings.SPACE_VOICE) && s.voiceInsertionReady();
                    press.gesture.down(SystemClock.uptimeMillis(),
                        KeyboardGeometry.fromPixels(s, event.getX()), voice);
                    press.cancelLongPress();
                    if (voice) {
                        press.longPress = () -> {
                            press.longPress = null;
                            press.gesture.tick(SystemClock.uptimeMillis());
                            if (press.gesture.state() == SpaceGesturePolicy.State.VOICE
                                    && s.voiceInsertionReady()) startVoice.run();
                        };
                        s.main.postDelayed(press.longPress, SpaceGesturePolicy.LONG_PRESS_MS);
                    }
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (voiced[0]) return true;
                    if (!dragging[0] && !cancelled[0]) {
                        float horizontal = event.getX() - origin[0];
                        float vertical = event.getY() - origin[1];
                        press.gesture.move(SystemClock.uptimeMillis(),
                            KeyboardGeometry.fromPixels(s, event.getX()));
                        SpaceGesturePolicy.State state = press.gesture.state();
                        if (state == SpaceGesturePolicy.State.VOICE) {
                            // VOICE is only reachable when long-press voice was on at ACTION_DOWN.
                            press.cancelLongPress();
                            if (s.voiceInsertionReady()) startVoice.run();
                            return true;
                        }
                        if (state == SpaceGesturePolicy.State.PRESSED) {
                            if (Math.abs(vertical) > touchSlop && Math.abs(vertical) >= Math.abs(horizontal)) {
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
                    SpaceGesturePolicy.Outcome outcome = press.gesture.up(SystemClock.uptimeMillis());
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    button.setPressed(false);
                    if (!voiced[0] && !dragging[0] && !cancelled[0]
                            && outcome == SpaceGesturePolicy.Outcome.VOICE
                            && touchPreference(AndroidLocalSettings.SPACE_VOICE)
                            && s.voiceInsertionReady()) {
                        // 计时回调还没来得及跑就松手了：仍按长按处理。
                        startVoice.run();
                    }
                    if (voiced[0]) {
                        s.countKey(button);
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
                    press.gesture.cancel();
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    dragging[0] = false;
                    cancelled[0] = true;
                    resetSpaceCursor();
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    Button actionRowKey(KeyboardActionRow.DesignSlot slot) {
        return switch (slot) {
            case LAYER -> s.layerButton;
            case LANGUAGE -> s.languageButton;
            case GLOBE -> s.globeButton;
            case COMMA -> s.quickPunctuationButton;
            case SPACE -> s.spaceButton;
            case PERIOD -> periodButton();
            case RETURN -> s.enterButton;
        };
    }

    /** 底行句号键：中文标点模式画「。」，否则「.」；发出的都是 `.`，由 Engine 按标点模式转换。 */
    private Button periodButton() {
        if (periodButton == null) {
            periodButton = s.keyId(s.keyboardKey(".", "句号", () -> s.type('.')), "Period");
            KeyboardGeometry.setKeyTextSize(periodButton, 22);
        }
        return periodButton;
    }

    /** 新设计的 123 / #+= 层自带底行，这时功能行整行不显示。 */
    boolean layerOwnsBottomRow() {
        return s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
            && ImeLetterRows.drawsDesignLayer(s.displayedTouchLayout(s.view));
    }

    /**
     * Lay the bottom row out for the surface on screen.
     *
     * <p>Every control here is a long-lived field with its own listeners and state, so the row is
     * re-parented rather than rebuilt: a fresh set of buttons each time would drop the space key's
     * cursor gesture and the delete key's repeat.
     */
    void updateActionRow() {
        if (s.actionRow == null) return;
        ensureFaces();
        s.imeStyler.refreshSeasonIfNeeded();
        s.imeLetterRows.updateSecondRowIndent();
        int layout = s.displayedTouchLayout(s.view);
        boolean globe = s.offersGlobeKey();
        boolean ownBottom = layerOwnsBottomRow();
        boolean chinesePunctuation = s.sendsChinesePunctuation();
        if (periodButton != null) {
            String periodFace = KeyboardActionRow.punctuationFace(
                KeyboardActionRow.DesignSlot.PERIOD, chinesePunctuation);
            // 全角「。」在字身里只占左下角一小块，按墨迹居中放大画，否则只看到键底一个小点。
            CenteredGlyphSpan.apply(periodButton, periodFace, 1.3f);
            periodButton.setContentDescription("按键 " + periodFace);
            // type('.') 没有会话时也会直接上屏字面句点（密码框等），与旁边的逗号键一致。
            ViewPolicy.setEnabled(periodButton, s.connection != null);
        }
        if (s.spaceButton instanceof SpaceKeyFace face) face.setSchemeLabel(spaceLabel());
        syncSplitSpace();
        if (s.globeButton != null) s.globeButton.setContentDescription("切换输入法");
        // Every keystroke reaches render(), and re-parenting eight keys under the pressed one is a
        // relayout the user can see. The row only changes when the surface does.
        // 分离时空格拆成两半、空隙按可见键居中，所以常用标点键显示与否也会改变排布。
        boolean split = !ownBottom && s.splitKeyboardDrawn();
        String signature = layout + ":" + globe + ":" + ownBottom + ":" + split
            + (split && s.quickPunctuationButton != null ? ":" + s.quickPunctuationButton.getVisibility() : "");
        java.util.List<KeyboardActionRow.DesignEntry> entries = ownBottom ? java.util.List.of()
            : KeyboardActionRow.designEntries(layout, globe);
        // Visibility is re-asserted every time: the reply surface hides this row and restores it
        // without the surface itself having changed.
        s.actionRow.setVisibility(entries.isEmpty() ? View.GONE : View.VISIBLE);
        if (signature.equals(s.actionRowSignature)) {
            if (!ownBottom) s.updateQuickPunctuation();
            return;
        }
        s.actionRowSignature = signature;
        s.actionRow.removeAllViews();
        // 符号面板入口挪到了 #+= 层左下，底行不再放「符」。
        if (s.symbolPanelButton != null && s.symbolPanelButton.getParent() == s.actionRow)
            s.actionRow.removeView(s.symbolPanelButton);
        for (KeyboardActionRow.DesignEntry entry : entries) {
            Button key = actionRowKey(entry.slot());
            if (key == null) continue;
            if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
            KeyboardKeyRole role = switch (entry.slot()) {
                case RETURN -> returnKeyRole();
                case SPACE, COMMA, PERIOD -> KeyboardKeyRole.KEY;
                default -> KeyboardKeyRole.ACCENT;
            };
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(role);
            // 123、中/英、逗号由通用的 button() 建出来，没有经过键帽工厂；放进底行时才知道它们是键帽。
            KeyboardGeometry.normalizeKeyCap(key);
            s.imeStyler.styleButton(key, role, s.skin);
            if (entry.slot() == KeyboardActionRow.DesignSlot.COMMA)
                KeyboardGeometry.setKeyTextSize(key, 22);
            else if (role == KeyboardKeyRole.ACCENT)
                KeyboardGeometry.setKeyTextSize(key, 15);
            key.setVisibility(View.VISIBLE);
            s.actionRow.addView(key, KeyboardGeometry.weightedMatchParentParams(entry.weight()));
        }
        // The quick punctuation key hides itself when the scheme has no punctuation to offer, and
        // the loop above just told every slot it was visible.
        if (!ownBottom) s.updateQuickPunctuation();
        if (split) s.imeLetterRows.splitSpaceRow(s.actionRow);
        s.imeStyler.applyKeyboardGeometry();
    }
}
