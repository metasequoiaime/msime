package app.msime.android;

import app.msime.android.LayoutPolicy;
import app.msime.android.core.InputViewValuePolicy;


import android.graphics.Color;
import android.util.TypedValue;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import app.msime.android.KeyboardGeometry;

/**
 * 字母键区（26 键、新设计的 123 层与 #+= 层）的行、删除键连删、按键气泡与下滑输入提示符。
 *
 * <p>⇧ 与 ⌫ 换成画描边图标的 {@link KeyboardIconKey}（节点 text 仍是 ⇧ / ⌫）：SVC 建的原按钮留作点击的执行者，新键点一下就让它 performClick，所以 Shift 的切换逻辑和删除动作留在 SVC 原处。
 */
final class ImeLetterRows {
    private final MSIMEInputService s;

    ImeLetterRows(MSIMEInputService s) {
        this.s = s;
    }

    /** 按键气泡的覆盖层：onCreateInputView 建好后盖在整个键盘上，初始为空。 */
    FrameLayout keyPreviewLayer;
    private KeyboardKeyPreview keyPreview;
    private KeyboardSkin keyPreviewSkin;
    private int keyPreviewBackground;
    private int keyPreviewForeground;
    private final int[] keyPreviewKeyLocation = new int[2];
    private final int[] keyPreviewLayerLocation = new int[2];
    /** 气泡当前跟随的键；多指交替时只有它的松手才收起气泡。 */
    private Button keyPreviewOwner;
    /** 符号层里正在显示 #+= 页（否则是 123 页）。 */
    private boolean moreSymbols;

    /** 数字页显示九宫格，不改变 Engine 里的中文方案和组码布局。 */
    boolean numberPadDrawn() {
        return numberPadDrawn(s.displayedTouchLayout(s.view));
    }

    boolean numberPadDrawn(int layout) {
        if (moreSymbols) return false;
        // 数字和电话框直接进入九宫格；文字密码框保留字母首页，点 123 时也使用九宫格。
        if (s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                && (s.directNumberPad() || EditorPolicy.password(s.editorInputType))) return true;
        boolean local = !"none".equals(InputViewValuePolicy.textOr(s.view, "local_mode", "none"));
        return KeyboardLayout.drawsNumberPad(s.keyboardLayer, layout,
            KeyboardLayout.numberPadLanguage(InputViewValuePolicy.scheme(s.view, -1), s.dedicatedEnglish), local);
    }

    /** 键行是否按另一种数字页状态建成：在借用的数字页上切中/英、退出本地模式等不改变触屏布局，也要换回原键面。 */
    boolean numberPadStale() {
        return s.keyRows != null && builtNumberPad != numberPadDrawn();
    }

    /** 数字页的扩展符号入口，返回 123 后仍回到数字九宫格。 */
    void showMoreSymbols() {
        moreSymbols = true;
        rebuildKeyRows();
        s.render();
    }
    /** 分离式键盘第二行（a–l）的缩进占位；微软双拼的第十个键出现时收起。 */
    private View secondRowLeadingIndent;
    private View secondRowTrailingIndent;
    private View eighteenRowLeadingIndent;
    private View eighteenRowTrailingIndent;
    /** 当前键行是不是按分离式键盘建的；与 {@link MSIMEInputService#splitKeyboardDrawn} 不一致时要重建（{@link #splitStale}）。 */
    private boolean builtSplit;
    /** 当前键行有没有画数字行；与 {@link MSIMEInputService#numberRowDrawn} 不一致时要重建（{@link #numberRowStale}）。 */
    private boolean builtNumberRow;
    private boolean builtSymbolExtraRow;
    /** 123 / #+= 层建的时候用的标点：全角中文或半角英文。中文标点开关变了要重建，否则键面和上屏的还是旧的。 */
    private Boolean builtLayerPunctuation;
    /** 数字页建行时顶部是否提供表情入口，变更后重建底行。 */
    private Boolean builtLayerToolbarEmoji;
    /** 当前键行若是 {@link #rebuildKeyRows} 的通用分支在字母层建的，记下按哪个布局建；其他分支建的为 -1。只有这种键行能就地换大小写（{@link #refreshLetterCase}）。 */
    private int builtLetterLayout = -1;
    /** 分离式键盘建行时第二行有没有算上微软双拼的 `;` 键（{@link #microsoftTenKeys}）；它决定两半的切分位置和内侧 G 是否重复，变了只能重建，不能就地换大小写。 */
    private boolean builtMicrosoftTenKeys;
    /** 通用分支建的韩文键帽和它们的 QWERTY 字母，Shift 换的是双辅音键面，就地改它们（{@link #refreshLetterCase}）。 */
    private final java.util.List<Button> koreanKeyButtons = new java.util.ArrayList<>();
    private final java.util.List<String> koreanKeyInputs = new java.util.ArrayList<>();

    /** 新设计的 123 / #+= 层画在哪些界面上：26 键（含韩文键面）、底行与 26 键相同的 14 键，以及把符号页交给 26 键行的手写、笔画和注音 9 键。这一层的字符键原样上屏、不经 Engine，所以注音 9 键的数字页不会被读成音键；大千注音的数字和标点键另有用途，保留原符号行。 */
    static boolean drawsDesignLayer(int touchLayout) {
        return touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
            || touchLayout == KeyboardLayout.KOREAN_LAYOUT
            || touchLayout == KeyboardLayout.FOURTEEN_KEY_LAYOUT
            || touchLayout == KeyboardLayout.EIGHTEEN_KEY_LAYOUT
            || touchLayout == KeyboardLayout.HANDWRITING_LAYOUT
            || touchLayout == KeyboardLayout.STROKE_LAYOUT
            || touchLayout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT;
    }

    /** 14 键第 3 行左端的分词键；键面随组字状态在「符」和「分词」之间换（{@link #updateFourteenKeySeparator}）。不在 14 键字母层时为 null。 */
    private Button fourteenSeparatorKey;
    private Button fourteenLKey;
    private Button shiftSource;

    /** 偏好里的一个布尔触控开关。 */
    private boolean touchPreference(String key) {
        return s.imeBottomRow.touchPreference(key);
    }

    /** 把 SVC 建的 ⇧ ⌫ 换成画图标的键（每次 onCreateInputView 新建控件后各换一次）。 */
    void ensureIconKeys() {
        if (s.shiftButton != null && !(s.shiftButton instanceof KeyboardIconKey)) {
            Button original = s.shiftButton;
            shiftSource = original;
            KeyboardIconKey key = iconKey(original, KeyboardIconKey.Kind.SHIFT,
                original.getText(), original.getContentDescription(), "ShiftLeft");
            s.shiftButton = key;
        }
        if (s.deleteButton != null && !(s.deleteButton instanceof KeyboardIconKey)) {
            Button original = s.deleteButton;
            KeyboardIconKey key = iconKey(original, KeyboardIconKey.Kind.BACKSPACE,
                "⌫", "删除", "Backspace");
            bindBackspaceRepeat(key, s::deleteFromHandwriting);
            s.deleteButton = key;
        }
    }

    /** 原按钮执行点击并计数，替身负责绘制；两个身份必须随分词状态一起恢复。 */
    void updateShiftKeyIdentity(boolean separator) {
        String id = separator ? "Quote" : "ShiftLeft";
        if (shiftSource != null) s.keyId(shiftSource, id);
        if (s.shiftButton != null) s.keyId(s.shiftButton, id);
        if (s.shiftButton instanceof KeyboardIconKey key) {
            key.setDrawsText(separator);
            KeyboardTypographyPolicy.setKeyTextSize(key,
                separator ? 14 : KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
        }
    }

    private KeyboardIconKey iconKey(Button original, KeyboardIconKey.Kind kind,
            CharSequence text, CharSequence description, String keyId) {
        KeyboardIconKey key = new KeyboardIconKey(s, kind);
        key.setText(text);
        key.setContentDescription(description);
        ViewPolicy.bindClick(key, original::performClick);
        s.keyId(key, keyId);
        s.imeKeyFeedback.stageFace(original, key);
        return key;
    }

    /** 按键气泡；第一次用时加进覆盖层。 */
    private KeyboardKeyPreview keyPreview() {
        if (keyPreviewLayer == null) return null;
        if (keyPreview == null || keyPreview.getParent() != keyPreviewLayer) {
            keyPreview = new KeyboardKeyPreview(s);
            keyPreviewLayer.addView(keyPreview, LayoutPolicy.frameWrapParams());
        }
        return keyPreview;
    }

    private void showKeyPreview(Button key, String label) {
        // 密码框不放大显示按下的字符，避免旁观者看到输入。
        if (EditorPolicy.password(s.editorInputType)) return;
        KeyboardKeyPreview preview = keyPreview();
        if (preview == null || !key.isAttachedToWindow()) return;
        KeyboardSkin skin = s.imeStyler.themed(s.skin);
        if (skin != keyPreviewSkin) {
            keyPreviewSkin = skin;
            keyPreviewBackground = Color.parseColor(skin.keyBackground());
            keyPreviewForeground = Color.parseColor(skin.keyForeground());
        }
        preview.setColors(keyPreviewBackground, keyPreviewForeground,
            ColorPolicy.withAlpha(Color.BLACK, 20));
        key.getLocationInWindow(keyPreviewKeyLocation);
        keyPreviewLayer.getLocationInWindow(keyPreviewLayerLocation);
        float weight = key.getLayoutParams() instanceof LinearLayout.LayoutParams params
            ? params.weight : 1f;
        preview.show(label, keyPreviewKeyLocation[0] - keyPreviewLayerLocation[0],
            keyPreviewKeyLocation[1] - keyPreviewLayerLocation[1], key.getWidth(), weight,
            keyPreviewLayer.getWidth());
        keyPreviewOwner = key;
    }

    /** 无条件收起气泡（整体复位用）。 */
    void hideKeyPreview() {
        if (keyPreview != null) keyPreview.hide();
        keyPreviewOwner = null;
    }

    /** 只在气泡仍属于这个键时收起，避免先松开的手指收掉另一根仍按着的键的气泡。 */
    private void hideKeyPreview(Button key) {
        if (keyPreviewOwner == key) hideKeyPreview();
    }

    /** 九键复用字母键的预览；数字滑动另行更新键面，松手与取消只收起本键的气泡。 */
    void updateKeyPreview(Button key, MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                if (touchPreference(AndroidLocalSettings.KEY_POPUP))
                    showKeyPreview(key, key.getText().toString());
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> hideKeyPreview(key);
            default -> { }
        }
    }

    /** 已显示的预览随滑动换成数字，不让关闭预览的设置被手势重新打开。 */
    void updateKeyPreviewLabel(Button key, String label) {
        if (keyPreviewOwner == key && keyPreview != null) keyPreview.setLabel(label);
    }

    /**
     * 字母键按压按「按键弹出」显示气泡。长按英文键提供大小写、上下标和声调，中文键提供小写、大写与角标，长按不受滑动开关影响。滑动输入符号启用时，沿设置方向滑过 14 dp 松手输入角标；中文反方向滑动打开选项并可继续拖选。关闭开关停用两种滑动，未触发手势时照常交给按钮点击。
     *
     * <p>中文标点生效时（{@link MSIMEInputService#sendsChinesePunctuation}），Engine 会转写的那几个提示（( ) " ? !，见 {@link LetterHintTable#routesThroughEngine}）气泡显示中文键面，松手交给 {@link MSIMEInputService#punctuation}：组字中先按首选结束再接中文标点，智能标点、成对补全和引号交替都照常生效（#7069）。这里不走 `type()`，一次性 Shift 不会被一个标点用掉，与按字面上屏的其他提示一致。中文标点状态在手势触发时读，不在建键时记下，切换中文标点不必为此重建键行。
     *
     * @param input 键的字母，提示字符按它查 {@link LetterHintTable}
     */
    private void bindLetterGestures(Button key, String input) {
        final String hint = LetterHintTable.hint(input);
        final float[] downY = new float[1];
        final boolean[] triggered = new boolean[1];
        final boolean[] optionsShown = new boolean[1];
        final boolean[] candidateTouch = new boolean[1];
        Runnable hold = () -> {
            if (triggered[0] || !key.isPressed()) return;
            // 候选栏出现时，长按也交给普通按键点击，避免误打开字母选项。
            if (!s.dedicatedEnglish && !s.letterCaseSchemeActive()
                    && ViewPolicy.isVisible(s.candidateViewport)) return;
            triggered[0] = true;
            optionsShown[0] = true;
            // 长按不看「按键弹出」：这是手势已触发的唯一提示。
            if (s.dedicatedEnglish) {
                hideKeyPreview(key);
                s.imeLayoutRows.showEnglishLetterOptions(key, input);
            } else if (!s.dedicatedEnglish && !s.letterCaseSchemeActive()) {
                hideKeyPreview(key);
                s.imeLayoutRows.showLetterOptions(key, input);
            } else {
                optionsShown[0] = false;
                showKeyPreview(key, LetterHintTable.face(input, s.sendsChinesePunctuation(), s.punctuationMarks()));
            }
        };
        key.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    downY[0] = DimensionPolicy.fromPixels(s, event.getY());
                    triggered[0] = false;
                    optionsShown[0] = false;
                    candidateTouch[0] = !s.dedicatedEnglish && !s.letterCaseSchemeActive()
                        && ViewPolicy.isVisible(s.candidateViewport);
                    s.imeLayoutRows.dismissNineKeyHoldOptions();
                    // 气泡读按下这一刻的键面：大小写变化是就地改键面（refreshLetterCase），建键时的键面可能已经过时。
                    if (touchPreference(AndroidLocalSettings.KEY_POPUP)) showKeyPreview(key, key.getText().toString());
                    if (hint != null) {
                        key.removeCallbacks(hold);
                        key.postDelayed(hold, ViewConfiguration.getLongPressTimeout());
                    }
                    return false;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (optionsShown[0]) {
                        s.imeLayoutRows.moveLetterOptions(key, event);
                    } else if (hint != null && !triggered[0]) {
                        boolean candidateVisible = candidateTouch[0]
                            || (!s.dedicatedEnglish && !s.letterCaseSchemeActive()
                                && ViewPolicy.isVisible(s.candidateViewport));
                        if (candidateVisible) return false;
                        if (!SwipeHintPolicy.enabled(
                                touchPreference(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS), false))
                            return false;
                        String direction = s.localSettings.choice(AndroidLocalSettings.SWIPE_SYMBOLS_DIRECTION);
                        String opposite = SwipeHintPolicy.UP.equals(direction) ? SwipeHintPolicy.DOWN : SwipeHintPolicy.UP;
                        float currentY = DimensionPolicy.fromPixels(s, event.getY());
                        if (SwipeHintPolicy.swiped(direction, downY[0], currentY)) {
                            triggered[0] = true;
                            key.removeCallbacks(hold);
                            if (keyPreview != null && keyPreviewOwner == key)
                                keyPreview.setLabel(LetterHintTable.face(input, s.sendsChinesePunctuation(), s.punctuationMarks()));
                        } else if (!s.dedicatedEnglish && !s.letterCaseSchemeActive()
                                && SwipeHintPolicy.swiped(opposite, downY[0], currentY)) {
                            triggered[0] = true;
                            optionsShown[0] = true;
                            key.removeCallbacks(hold);
                            hideKeyPreview(key);
                            s.imeLayoutRows.showLetterOptions(key, input);
                        }
                    }
                    return triggered[0];
                }
                case MotionEvent.ACTION_UP -> {
                    key.removeCallbacks(hold);
                    hideKeyPreview(key);
                    if (!triggered[0]) {
                        // 候选态的长按或滑动必须等价于一次普通点按，即使位移让 Button 自己取消了点击。
                        boolean candidateVisible = candidateTouch[0]
                            || (!s.dedicatedEnglish && !s.letterCaseSchemeActive()
                                && ViewPolicy.isVisible(s.candidateViewport));
                        if (candidateVisible) {
                            key.performClick();
                            key.setPressed(false);
                            return true;
                        }
                        return false;
                    }
                    MotionEvent cancel = MotionEvent.obtain(event);
                    cancel.setAction(MotionEvent.ACTION_CANCEL);
                    key.onTouchEvent(cancel);
                    cancel.recycle();
                    key.setPressed(false);
                    if (optionsShown[0] && s.imeLayoutRows.finishLetterOptions(key, event)) return true;
                    s.imeKeyFeedback.playFeedback(key);
                    s.countKey(key);
                    if (!optionsShown[0]) s.imeLayoutRows.commitLetterHint(input);
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    key.removeCallbacks(hold);
                    hideKeyPreview(key);
                    s.imeLayoutRows.cancelLetterOptions(key);
                    return false;
                }
                default -> { return false; }
            }
        });
    }

    /** 分离式键盘第二行的缩进只在 9 键行出现；微软双拼的第十个键可见时收起（每次渲染校正一次）。 */
    void updateSecondRowIndent() {
        if (secondRowLeadingIndent == null) return;
        int visibility = microsoftTenKeys() ? View.GONE : View.VISIBLE;
        if (secondRowLeadingIndent.getVisibility() != visibility) {
            if (visibility == View.VISIBLE) {
                ViewPolicy.show(secondRowLeadingIndent);
                ViewPolicy.show(secondRowTrailingIndent);
            } else {
                ViewPolicy.hide(secondRowLeadingIndent);
                ViewPolicy.hide(secondRowTrailingIndent);
            }
        }
    }

    /** 第二行此刻是否带着微软双拼的第十个键（`;`）；与 SVC 每次渲染时设置那个键可见性的判断相同。 */
    private boolean microsoftTenKeys() {
        String localMode = s.view == null ? "none"
            : JsonPolicy.strictStringOrEmpty(s.view.opt("local_mode"));
        if (localMode.isEmpty()) localMode = "none";
        return s.microsoftFinalKey != null && MicrosoftShuangpinKeyPolicy.visible(
            s.dedicatedEnglish, s.selectedScheme, localMode);
    }

    /** 键行是否需要按分离式键盘的新状态重建：旋转、开关变化或换到另一种布局之后。 */
    boolean splitStale() {
        return s.keyRows != null && builtSplit != s.splitKeyboardDrawn();
    }

    /** 键行是否需要按「数字行」的新状态重建：设置页改了开关、或旋转让窗口高度跨过了 {@link KeyboardLayout#NUMBER_ROW_MIN_WINDOW_HEIGHT_DP} 之后。 */
    boolean numberRowStale() {
        return s.keyRows != null && (builtNumberRow != s.numberRowDrawn()
            || builtSymbolExtraRow != s.symbolExtraRowDrawn());
    }

    /** 正显示着 123 / #+= 层，而标点或顶部表情入口和建层时不一样了。 */
    boolean layerPunctuationStale() {
        return s.keyRows != null && builtLayerPunctuation != null
            && s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
            && (builtLayerPunctuation != s.sendsChinesePunctuation()
                || (builtLayerToolbarEmoji != null && builtLayerToolbarEmoji != (!s.toolbarHidden && s.toolbarEmoji)));
    }

    /** 分离式键盘中间的空隙：普通的 View，不是键、不可点击，也不带键距外边距，所以 {@link KeyboardKeyArea} 不会把落在这里的按下交给旁边的键，点它什么也不发生。 */
    private View splitGap() {
        View gap = new View(s);
        ViewPolicy.hideFromAccessibility(gap);
        return gap;
    }

    /** 一个子视图在分离计算里的份额：按渲染后的最终可见状态计，隐藏的记 0；微软双拼的 `;` 键在建行时还没被渲染校正，按 {@link #microsoftTenKeys} 计。 */
    private float splitWeight(View child) {
        if (child == s.microsoftFinalKey) return microsoftTenKeys() ? 1f : 0f;
        if (ViewPolicy.isGone(child)) return 0f;
        return child.getLayoutParams() instanceof LinearLayout.LayoutParams params ? params.weight : 0f;
    }

    private float[] splitWeights(LinearLayout row) {
        float[] weights = new float[row.getChildCount()];
        for (int index = 0; index < weights.length; index++) weights[index] = splitWeight(row.getChildAt(index));
        return weights;
    }

    private static float sum(float[] weights) {
        float total = 0f;
        for (float weight : weights) total += weight;
        return total;
    }

    /** 在键边界上把一行分成左右两半，中间插入占整行 25% 的空隙（断点规则见 {@link SplitKeyboardPolicy#cutIndex}）。 */
    void splitRow(LinearLayout row) {
        splitRow(row, 0f);
    }

    /** 同 {@link #splitRow(LinearLayout)}，空隙里要再放一个份额为 {@code duplicateWeight} 的重复键（{@link SplitKeyboardPolicy#gapWeight(float, float)}）。 */
    private void splitRow(LinearLayout row, float duplicateWeight) {
        float[] weights = splitWeights(row);
        row.addView(splitGap(), SplitKeyboardPolicy.cutIndex(weights), LayoutPolicy.weightedMatchParentParams(
            SplitKeyboardPolicy.gapWeight(sum(weights), duplicateWeight)));
    }

    /**
     * 底行（字母层的功能行、123 / #+= 层自带的底行）不在键边界上断，而是把空格键拆成两个：原来的空格键留在左半边的内侧，{@link ImeBottomRow#splitSpaceButton} 放在右半边的内侧，两者都是空格，长按语音和拖动移光标也都在。左边那一半的宽度让空隙落在整行正中（{@link SplitKeyboardPolicy#leftSpaceWeight}）。行里没有空格键时按普通行处理。
     */
    void splitSpaceRow(LinearLayout row) {
        Button space = s.spaceButton;
        int spaceIndex = space == null ? -1 : row.indexOfChild(space);
        if (spaceIndex < 0) {
            splitRow(row);
            return;
        }
        float[] weights = splitWeights(row);
        float total = sum(weights);
        float left = SplitKeyboardPolicy.leftSpaceWeight(weights, spaceIndex);
        float right = weights[spaceIndex] - left;
        space.setLayoutParams(LayoutPolicy.weightedMatchParentParams(left));
        row.addView(splitGap(), spaceIndex + 1,
            LayoutPolicy.weightedMatchParentParams(SplitKeyboardPolicy.gapWeight(total)));
        Button second = s.imeBottomRow.splitSpaceButton();
        if (second.getParent() instanceof android.view.ViewGroup parent) parent.removeView(second);
        ViewPolicy.show(second);
        row.addView(second, spaceIndex + 2, LayoutPolicy.weightedMatchParentParams(right));
    }

    private View indent() {
        View spacer = new View(s);
            ViewPolicy.hideFromAccessibility(spacer);
        return spacer;
    }

    /** 这一次删除键按压的滑动阶段，见 {@link BackspaceSwipePolicy}。 */
    private BackspaceSwipePolicy.Phase backspaceSwipePhase = BackspaceSwipePolicy.Phase.IDLE;
    private boolean backspaceUndo;
    private boolean backspaceUndoHeld;
    private BackspaceScrub backspaceScrub;
    private float backspaceDownX;
    private float backspaceDownY;
    private float backspaceTravel;
    private float backspaceActivation;
    private int backspaceInitialDeleted;
    private boolean backspaceHorizontal;

    void recordBackspaceScrubDelete(CharSequence before) {
        if (backspaceScrub != null) backspaceScrub.ordinaryDelete(before);
    }

    void invalidateBackspaceScrub() {
        if (backspaceScrub != null) backspaceScrub.invalidate();
    }

    /**
     * 短按删一次，按住连删并逐级加速；左滑锁定水平删除，未松手时右滑恢复（#9324）。上下滑仍提供快速删除（#5585）与撤回（#8325），方向一旦锁定就不切换。
     */
    void bindBackspaceRepeat(Button button, Runnable action) {
        button.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    cancelBackspaceRepeat();
                    s.backspaceRepeatButton = button;
                    s.backspaceRepeated = false;
                    s.backspaceClearedComposition = false;
                    backspaceUndo = s.canUndoQuickDelete();
                    backspaceDownX = event.getX();
                    backspaceDownY = event.getY();
                    int[] keyLocation = new int[2];
                    int[] rootLocation = new int[2];
                    button.getLocationOnScreen(keyLocation);
                    if (s.keyboardRoot != null) s.keyboardRoot.getLocationOnScreen(rootLocation);
                    backspaceTravel = Math.max(32f * DimensionPolicy.density(s),
                        keyLocation[0] + event.getX() - rootLocation[0] - 8f * DimensionPolicy.density(s));
                    if (!backspaceUndo) backspaceScrub = s.beginBackspaceScrub();
                    button.setPressed(true);
                    // 没有备份时按下即删并计数；有备份先等到松手，免得下滑撤回之前又破坏恢复位置。
                    if (!backspaceUndo) button.performClick();
                    s.backspaceRepeatTask = new Runnable() {
                        private int repeats;

                        @Override public void run() {
                            if (s.backspaceRepeatButton != button || !button.isPressed()) return;
                            if (backspaceUndo) {
                                backspaceUndoHeld = true;
                                backspaceSwipePhase = BackspaceSwipePolicy.Phase.REVEALED;
                                if (s.quickDeleteOverlay != null) s.quickDeleteOverlay.show(button, false, true);
                                return;
                            }
                            // 连删无论重复几次都算一次按压，按下时已经计数。
                            s.backspaceRepeated = true;
                            if (s.hasEngineComposition()) {
                                s.imeKeyFeedback.playFeedback(button);
                                // 长按删除整个音节，汉字列表打开时也一样。
                                s.discardComposition();
                                s.backspaceClearedComposition = true;
                                s.main.removeCallbacks(this);
                                s.backspaceRepeatTask = null;
                                return;
                            }
                            if (repeats == 0 && s.quickDeleteOverlay != null
                                    && button.getWindowToken() != null
                                    && BackspaceSwipePolicy.available(s.quickDeleteOverlay.keyTop(button),
                                        DimensionPolicy.density(s))) {
                                // 长按先提示可上滑清空；手指仍在键内时保持连删，松手不能直接清空。
                                s.quickDeleteOverlay.show(button, false);
                            }
                            s.imeKeyFeedback.playFeedback(button);
                            action.run();
                            repeats++;
                            s.main.postDelayed(this, BackspaceRepeatPolicy.repeatInterval(repeats));
                        }
                    };
                    s.main.postDelayed(s.backspaceRepeatTask, BackspaceRepeatPolicy.INITIAL_DELAY_MS);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (s.backspaceRepeatButton != button) return true;
                    float dx = backspaceDownX - event.getX();
                    if (!backspaceHorizontal && backspaceScrub != null && backspaceScrub.available()
                            && backspaceSwipePhase == BackspaceSwipePolicy.Phase.IDLE
                            && BackspaceScrub.starts(dx, event.getY() - backspaceDownY, DimensionPolicy.density(s))) {
                        backspaceHorizontal = true;
                        backspaceActivation = 8f * DimensionPolicy.density(s);
                        backspaceInitialDeleted = Math.max(1, backspaceScrub.deleted());
                        stopBackspaceRepeatTask();
                        button.setPressed(false);
                        if (s.quickDeleteOverlay != null) s.quickDeleteOverlay.hide();
                        s.imeKeyFeedback.playFeedback(button);
                    }
                    if (backspaceHorizontal) {
                        int deleted = BackspaceScrub.deletionCount(dx, backspaceTravel, backspaceScrub.count(),
                            backspaceActivation, backspaceInitialDeleted);
                        s.moveBackspaceScrub(backspaceScrub, deleted);
                        return true;
                    }
                    if (backspaceUndo) {
                        if (backspaceUndoHeld) {
                            BackspaceSwipePolicy.Phase next = BackspaceSwipePolicy.undoPhase(
                                event.getY(), button.getHeight(), DimensionPolicy.density(s));
                            if (next != backspaceSwipePhase && next == BackspaceSwipePolicy.Phase.ARMED)
                                s.imeKeyFeedback.playFeedback(button);
                            backspaceSwipePhase = next;
                            if (s.quickDeleteOverlay != null)
                                s.quickDeleteOverlay.show(button, next == BackspaceSwipePolicy.Phase.ARMED, true);
                        } else if (event.getX() < 0 || event.getX() >= button.getWidth()
                                || event.getY() < 0 || event.getY() >= button.getHeight()) {
                            // 未到长按时离开键，取消这次点击；不能顺便删掉备份的位置。
                            stopBackspaceRepeatTask();
                            button.setPressed(false);
                        }
                        return true;
                    }
                    if (trackBackspaceSwipe(button, event.getY())) return true;
                    if (event.getX() < 0 || event.getY() < 0
                            || event.getX() >= button.getWidth()
                            || event.getY() >= button.getHeight()) {
                        // 手指离开键就停止连删，但仍跟踪这次按压：接着往上滑还能弹出快速删除框。
                        stopBackspaceRepeatTask();
                        button.setPressed(false);
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    boolean clear = s.backspaceRepeatButton == button
                        && BackspaceSwipePolicy.clearsOnRelease(backspaceSwipePhase);
                    boolean undo = backspaceUndo;
                    boolean click = undo && !backspaceUndoHeld && button.isPressed();
                    cancelBackspaceRepeat();
                    button.setPressed(false);
                    if (clear) {
                        s.imeKeyFeedback.playFeedback(button);
                        if (undo) s.undoQuickDelete();
                        else s.deleteAllBeforeCursor();
                    }
                    if (click) button.performClick();
                    return true;
                }
                case MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_OUTSIDE -> {
                    cancelBackspaceRepeat();
                    button.setPressed(false);
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    /**
     * 按 {@link BackspaceSwipePolicy} 更新上滑阶段并画框；返回 true 表示这次移动已经属于上滑手势（框弹出过）。第一次弹出时停掉连删，进入待命时给一次按键反馈。
     */
    private boolean trackBackspaceSwipe(Button button, float yInKey) {
        QuickDeleteOverlay overlay = s.quickDeleteOverlay;
        if (overlay == null || button.getWindowToken() == null) return false;
        float keyTop = overlay.keyTop(button);
        BackspaceSwipePolicy.Phase next = BackspaceSwipePolicy.next(backspaceSwipePhase,
            keyTop + yInKey, keyTop, DimensionPolicy.density(s));
        if (next == BackspaceSwipePolicy.Phase.IDLE) return false;
        BackspaceSwipePolicy.Phase previous = backspaceSwipePhase;
        backspaceSwipePhase = next;
        if (previous == BackspaceSwipePolicy.Phase.IDLE) {
            backspaceScrub = null;
            stopBackspaceRepeatTask();
            button.setPressed(false);
        }
        if (next != previous) {
            if (next == BackspaceSwipePolicy.Phase.ARMED) s.imeKeyFeedback.playFeedback(button);
            overlay.show(button, next == BackspaceSwipePolicy.Phase.ARMED);
        }
        return true;
    }

    private void stopBackspaceRepeatTask() {
        if (s.backspaceRepeatTask != null) s.main.removeCallbacks(s.backspaceRepeatTask);
        s.backspaceRepeatTask = null;
    }

    void cancelBackspaceRepeat() {
        stopBackspaceRepeatTask();
        if (s.backspaceRepeatButton != null) s.backspaceRepeatButton.setPressed(false);
        s.backspaceRepeatButton = null;
        s.backspaceRepeated = false;
        s.backspaceClearedComposition = false;
        backspaceSwipePhase = BackspaceSwipePolicy.Phase.IDLE;
        backspaceUndo = false;
        backspaceUndoHeld = false;
        backspaceScrub = null;
        backspaceHorizontal = false;
        if (s.quickDeleteOverlay != null) s.quickDeleteOverlay.hide();
    }

    /**
     * 大小写变了（Shift、一次性大写用掉、自动大写，或 Shift 顺带从中文切到英文）之后更新字母键。键行是通用分支在字母层建的，且布局、分离式键盘、数字行都和建的时候一样时，就地改每个字母键的键面和无障碍描述；否则整组重建。
     *
     * <p>不重建是 #6997 第 5 项的修法：重建会先 `removeAllViews` 再建新键，新键在下一次布局之前宽高都是 0，这段时间里落下的按压找不到键被整个丢掉；按在旧键上的手指也会因为旧键被摘下而收不到抬起。大小写只改键面，不改键的个数和位置，所以原来的键可以留着。
     */
    void refreshLetterCase() {
        if (!relabelLetterCase()) rebuildKeyRows();
    }

    private boolean relabelLetterCase() {
        if (s.keyRows == null || builtLetterLayout < 0
                || s.keyboardLayer != KeyboardLayout.Layer.LETTERS
                || s.displayedTouchLayout(s.view) != builtLetterLayout
                || splitStale() || numberRowStale()
                || builtSplit && microsoftTenKeys() != builtMicrosoftTenKeys) return false;
        // 与 rebuildKeyRows 通用分支里算键面的四项相同；中英和本地模式变了也只影响键面。
        boolean chineseMode = !s.dedicatedEnglish && !s.letterCaseSchemeActive();
        boolean japanese = s.japaneseSchemeActive();
        boolean localMode = s.view != null
            && !"none".equals(JsonPolicy.strictStringOrEmpty(s.view.opt("local_mode")));
        boolean shifted = s.letterCase.usesUppercase();
        for (int index = 0; index < s.shuangpinKeyButtons.size(); index++) {
            String input = s.shuangpinKeyInputs.get(index);
            if (";".equals(input)) continue;
            Button key = s.shuangpinKeyButtons.get(index);
            key.setText(LetterKeyFacePolicy.face(input, chineseMode, japanese, localMode, shifted));
            key.setContentDescription(LetterKeyFacePolicy.accessibilityLabel(
                input, chineseMode, localMode, shifted));
        }
        for (int index = 0; index < koreanKeyButtons.size(); index++) {
            String input = koreanKeyInputs.get(index);
            Button key = koreanKeyButtons.get(index);
            key.setText(KoreanKeyboardLayout.face(input, shifted));
            key.setContentDescription(KoreanKeyboardLayout.accessibilityLabel(input, shifted));
        }
        return true;
    }

    private int drawnTouchLayout = -1;

    /** 底行跟随已经构建的主键区，不提前采用尚未重建键区的会话快照。 */
    int drawnTouchLayout() {
        return drawnTouchLayout < 0 ? s.displayedTouchLayout(s.view) : drawnTouchLayout;
    }

    private boolean builtLanguageNumberSwap;
    private boolean builtNumberPad;

    boolean languageNumberSwapStale() {
        return builtLanguageNumberSwap != s.localSettings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS);
    }

    void rebuildKeyRows() {
        if (s.keyRows == null) return;
        drawnTouchLayout = s.displayedTouchLayout(s.view);
        builtLanguageNumberSwap = s.localSettings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS);
        builtLayerPunctuation = null;
        builtLayerToolbarEmoji = null;
        builtLetterLayout = -1;
        koreanKeyButtons.clear();
        koreanKeyInputs.clear();
        // 字母数字行在下方构建时记上；符号补充行独立记录，避免提前返回的网格分支丢失第五排。
        builtNumberRow = false;
        builtSymbolExtraRow = s.symbolExtraRowDrawn();
        boolean split = s.splitKeyboardDrawn();
        if (split != builtSplit) {
            builtSplit = split;
            // 分离式键盘铺满宽度，其余情况回到大屏的限宽外框；单手模式在分离时让位，换回来时恢复。
            s.imeStyler.applyKeyboardSurfaceGeometry();
            s.imeFrame.applyOneHanded();
        }
        ensureIconKeys();
        s.imeBottomRow.ensureFaces();
        hideKeyPreview();
        secondRowLeadingIndent = null;
        secondRowTrailingIndent = null;
        eighteenRowLeadingIndent = null;
        eighteenRowTrailingIndent = null;
        // 数字输入框沿用同一个输入法服务实例时，不能继承上一个文本框停留的 #+= 页。
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS || s.directNumberPad()) moreSymbols = false;
        builtNumberPad = numberPadDrawn();
        s.imeLayoutRows.hideJapaneseFlickPreview();
        s.deactivateHandwriting();
        s.symbolKeyButtons.clear();
        s.symbolKeyInputs.clear();
        s.shuangpinKeyButtons.clear();
        s.shuangpinKeyInputs.clear();
        s.microsoftFinalKey = null;
        s.nineKeySidebar = null;
        s.nineKeySymbolKey = null;
        s.imeLayoutRows.forgetNineKeyGrid();
        s.strokeWildcardKey = null;
        s.japaneseSpaceKey = null;
        s.japaneseReturnKey = null;
        s.japaneseSymbolsKey = null;
        s.japaneseVariantsButton = null;
        fourteenSeparatorKey = null;
        fourteenLKey = null;
        s.keyRows.removeAllViews();
        if (builtSymbolExtraRow) addSymbolExtraRow();
        if (s.displayedTouchLayout(s.view) == MSIMEInputService.JAPANESE_NINE_KEY_LAYOUT) {
            s.imeLayoutRows.rebuildJapaneseNineKeyRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        // 中文数字入口复用九宫格；字母层仍按原方案构建，扩展符号页另走通用符号行。
        if (numberPadDrawn() || (s.keyboardLayer == KeyboardLayout.Layer.LETTERS
                && s.displayedTouchLayout(s.view) == MSIMEInputService.QUANPIN_NINE_KEY_LAYOUT)) {
            s.imeLayoutRows.rebuildNineKeyRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == MSIMEInputService.HANDWRITING_LAYOUT) {
            s.imeLayoutRows.rebuildHandwritingRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        // 笔画字母层画笔画网格，数字层已由前面的数字九宫格接管。
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.STROKE_LAYOUT) {
            s.imeLayoutRows.rebuildStrokeRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        // 注音 9 键只在字母层画音键，数字层直接上屏数字，不交给引擎组音。
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT) {
            s.imeLayoutRows.rebuildZhuyinNineKeyRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        // 字母组网格只在字母层画三行，数字层与二十六键复用同一个九宫格。
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && KeyboardLayout.drawsLetterGrid(s.displayedTouchLayout(s.view))) {
            rebuildFourteenKeyRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        if (s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                && (drawsDesignLayer(s.displayedTouchLayout(s.view))
                    || s.displayedTouchLayout(s.view) == MSIMEInputService.QUANPIN_NINE_KEY_LAYOUT)) {
            rebuildDesignLayer();
            s.imeBottomRow.updateActionRow();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        // 越南语字母和藏文的威利转写字母按敲下的大小写写入，所以键面像英文键一样显示大小写，而不是中文键盘的大写键面；日语罗马字也不画大写键面，见 LetterKeyFacePolicy.displaysUppercase。
        boolean chineseMode = !s.dedicatedEnglish && !s.letterCaseSchemeActive();
        boolean japanese = s.japaneseSchemeActive();
        boolean localMode = s.view != null
            && !"none".equals(JsonPolicy.strictStringOrEmpty(s.view.opt("local_mode")));
        boolean shifted = s.letterCase.usesUppercase();
        boolean koreanKeycaps = s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.KOREAN_LAYOUT;
        boolean zhuyinLayout = s.displayedTouchLayout(s.view) == KeyboardLayout.ZHUYIN_LAYOUT;
        boolean zhuyinKeycaps = zhuyinLayout && s.keyboardLayer == KeyboardLayout.Layer.LETTERS;
        // 新设计的字母键：22 sp 键面，26 键（不含韩文与注音键面）右上角画提示符。长按总能输入它，所以关掉「滑动输入符号」也照画。
        boolean standardLetters = s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && !koreanKeycaps && !zhuyinKeycaps;
        // The face is the policy's job; the key itself always sends its canonical lowercase form.
        java.util.List<java.util.List<String>> rows = KeyboardLayout.rows(s.keyboardLayer,
            s.displayedTouchLayout(s.view));
        // 大千注音四行各占完整键高，避免压成三行后按键过矮；其他多行布局沿用三行高的整块。
        LinearLayout block = null;
        if (rows.size() > 3) {
            int blockRows = zhuyinKeycaps ? rows.size() : 3;
            block = LayoutPolicy.column(s);
            s.imeStyler.adjustRowBlockHeight(block, blockRows);
            s.keyRows.addView(block, LayoutPolicy.matchWidthHeightPx(
                s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * blockRows)));
        }
        boolean tibetanSymbols = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
            && s.tibetanSchemeActive();
        // 一行里的一个字符键。分离式键盘在右半边重复的 G、V（#6022）也从这里建，和左半边那个是同一种键。
        java.util.function.Function<String, Button> characterKey = rowKey -> {
            // 藏文的符号页把 `=` 换成叠写用的 `+`。
            final String key = KeyboardLayout.symbolRowKey(rowKey, tibetanSymbols);
            final String input = key;
            String face = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                ? ChineseSymbolFaces.face(key, s.sendsChinesePunctuation(), s.punctuationMarks())
                : koreanKeycaps ? KoreanKeyboardLayout.face(key, shifted)
                : zhuyinKeycaps ? ZhuyinKeyboardLayout.face(key)
                : LetterKeyFacePolicy.face(key, chineseMode, japanese, localMode, shifted);
            Button keyButton;
            if (zhuyinKeycaps) {
                // 键面只显示注音符号；type() 仍发送大千 ASCII 键，由引擎组音与转换。
                keyButton = s.keyboardKey(face, face, () -> s.type(input.charAt(0)));
                keyButton.setContentDescription(ZhuyinKeyboardLayout.accessibilityLabel(key));
                KeyboardTypographyPolicy.setFittedKeyTextSize(keyButton, 18, 22);
                if (ZhuyinKeyboardLayout.toneKey(key)) CenteredGlyphSpan.apply(keyButton, face, 1.8f);
                if (keyButton instanceof KeyboardPressButton press)
                    press.setKeyboardRole(KeyboardKeyRole.KEY);
            } else if (zhuyinLayout && ZhuyinKeyboardLayout.claimsSymbol(key)) {
                // Dachen reads these digits and marks as bopomofo and tone keys, so the symbol page writes what its key shows instead of handing the key to the Engine.
                keyButton = s.keyboardKey(face, face, () -> s.imeLayoutRows.commitNineKeyLiteral(
                    ChineseSymbolFaces.face(input, s.sendsChinesePunctuation(), s.punctuationMarks())));
            } else if (koreanKeycaps) {
                // Jamo keycaps over the same QWERTY letters: type() sends the letter, and the Engine composes the syllable.
                keyButton = s.keyboardKey(face, face, () -> s.type(input.charAt(0)));
                keyButton.setContentDescription(
                    KoreanKeyboardLayout.accessibilityLabel(key, shifted));
                if (keyButton instanceof KeyboardPressButton press)
                    press.setKeyboardRole(KeyboardKeyRole.KEY);
                koreanKeyButtons.add(keyButton);
                koreanKeyInputs.add(input);
            } else if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS) {
                ShuangpinHintButton hintButton = s.shuangpinKeyboardKey(
                    face, face, () -> s.type(input.charAt(0)));
                keyButton = hintButton;
                s.shuangpinKeyButtons.add(hintButton);
                s.shuangpinKeyInputs.add(input);
                // 键面随中文标点切换，之后由 render() 里的 updateLetterCornerHints 就地更新。
                hintButton.setCornerHint(standardLetters
                    ? LetterHintTable.face(input, s.sendsChinesePunctuation(), s.punctuationMarks()) : null);
                KeyboardTypographyPolicy.setKeyTextSize(hintButton, 22);
                bindLetterGestures(hintButton, standardLetters ? input : null);
            } else {
                keyButton = s.keyboardKey(face, face, () -> s.type(input.charAt(0)));
            }
            s.keyId(keyButton, KeyPressIds.forCharacter(input.charAt(0)));
            if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS && !koreanKeycaps && !zhuyinKeycaps) {
                keyButton.setContentDescription(LetterKeyFacePolicy.accessibilityLabel(
                    input, chineseMode, localMode, shifted));
                // 字母键读作「字母 Q」而不是「按键 Q」，所以描述推导一直把它判成 action 面，
                // 26 键的字母因此是实心深绿的。角色说了算之后就不必靠描述去猜。
                if (keyButton instanceof KeyboardPressButton press)
                    press.setKeyboardRole(KeyboardKeyRole.KEY);
            }
            if (s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS) {
                s.symbolKeyButtons.add(keyButton);
                s.symbolKeyInputs.add(input);
            }
            return keyButton;
        };
        // 数字行（#6022）：设置打开时在 26 键和韩文键盘的字母上方加一行 1–0，键盘整体高出一行。
        // 只在三行字母的 26 键和韩文键盘上画（KeyboardLayout.drawsNumberRow），这时没有四行挤三行的 block。
        builtNumberRow = s.numberRowDrawn();
        if (builtNumberRow) addNumberRow(rows.size());
        for (int rowIndex = 0; rowIndex < rows.size(); rowIndex++) {
            java.util.List<String> keys = rows.get(rowIndex);
            LinearLayout row = LayoutPolicy.row(s);
            if (block != null) {
                block.addView(row, LayoutPolicy.weightedWidthParams(1));
            } else {
                row.setTag(new MSIMEInputService.KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP,
                    rows.size(), rowIndex, true));
                s.keyRows.addView(row, LayoutPolicy.matchWidthWrapParams());
            }
            // 普通键盘第二行铺满宽度；分离式键盘保留两侧半键占位，使两半的键宽一致。
            if (builtSplit && standardLetters && rowIndex == 1) {
                secondRowLeadingIndent = indent();
                row.addView(secondRowLeadingIndent, LayoutPolicy.weightedMatchParentParams(.5f));
            }
            java.util.List<View> rowKeys = new java.util.ArrayList<>(keys.size());
            for (String rowKey : keys) {
                Button keyButton = characterKey.apply(rowKey);
                rowKeys.add(keyButton);
                row.addView(keyButton, LayoutPolicy.weightedMatchParentParams(1));
            }
            // The Dachen rows carry their own ; key (ㄤ), and no double-pinyin final.
            if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS && rowIndex == 1 && !zhuyinKeycaps) {
                s.microsoftFinalKey = s.keyId(s.shuangpinKeyboardKey(";", "微软双拼 ing", () -> s.type(';')),
                    "Semicolon");
                s.shuangpinKeyButtons.add((ShuangpinHintButton) s.microsoftFinalKey);
                s.shuangpinKeyInputs.add(";");
                KeyboardTypographyPolicy.setKeyTextSize(s.microsoftFinalKey, 22);
                row.addView(s.microsoftFinalKey, LayoutPolicy.weightedMatchParentParams(1));
            }
            if (builtSplit && standardLetters && rowIndex == 1) {
                secondRowTrailingIndent = indent();
                row.addView(secondRowTrailingIndent, LayoutPolicy.weightedMatchParentParams(.5f));
                updateSecondRowIndent();
            }
            // 大小写和删除属于最后一行的两端，不属于底部功能行。Leaving them in a strip below the keys
            // is what pushed every other control out of reach of a thumb.
            if (rowIndex == rows.size() - 1) {
                int layout = s.displayedTouchLayout(s.view);
                boolean symbols = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
                // 大千注音的末行本来就有 10 个键、也没有大小写键，⌫ 只占一个键的份额（KeyboardActionRow.letterRowEdgeWeight），不再挤窄 ㄈ…ㄥ。
                float edge = symbols || layout == KeyboardLayout.ZHUYIN_LAYOUT
                    ? KeyboardActionRow.letterRowEdgeWeight(layout, symbols)
                    : KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
                if (KeyboardActionRow.rowsCarryCase(layout, symbols))
                    addLetterRowEdgeKey(row, s.shiftButton, 0, edge);
                if (KeyboardActionRow.rowsCarryDelete(layout, symbols))
                    addLetterRowEdgeKey(row, s.deleteButton, row.getChildCount(), edge);
            }
            if (builtSplit && block == null) {
                // 字母层的第二、三行在右半边内侧重复左半边最内侧的那个字母（G、V，#6022），重复的键占掉空隙的一个键宽，三行的键宽不变；重复后两半不等宽时（微软双拼显示 ; 的第二行）不重复。
                float[] weights = splitWeights(row);
                boolean duplicate = s.keyboardLayer == KeyboardLayout.Layer.LETTERS
                    && SplitKeyboardPolicy.duplicatesInnerKey(rowIndex, rows.size(), weights);
                int cut = SplitKeyboardPolicy.cutIndex(weights);
                int inner = duplicate && cut > 0 ? rowKeys.indexOf(row.getChildAt(cut - 1)) : -1;
                splitRow(row, inner >= 0 ? 1f : 0f);
                if (inner >= 0) row.addView(characterKey.apply(keys.get(inner)), cut + 1,
                    LayoutPolicy.weightedMatchParentParams(1));
            }
        }
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS) {
            builtLetterLayout = s.displayedTouchLayout(s.view);
            builtMicrosoftTenKeys = microsoftTenKeys();
        }
        s.imeStyler.applyKeyboardGeometry();
    }

    /**
     * 全拼和双拼 14 键的字母层：{@link FourteenKeyLayout} 的三行，和 26 键字母行同样三行等高、按同样的方式分摊键盘高度调整，底行仍是 26 键的功能行，所以键盘总高与 26 键相同。
     *
     * <p>点按经 {@link MSIMEInputService#gridKey} 送这一组的首字母，不走 {@link MSIMEInputService#type}（辅助码、微软双拼 `;`、英文直出都按单个确定的字母处理）。第 2 行不缩进，双拼的分号并入 L 键；第 3 行左端是分词键（组字时送 `'`，空闲时打开符号面板，照搬九键 1 键），右端是 ⌫，两端宽度沿用 26 键 ⇧ ⌫ 的份额。中文 14 键没有 Shift、角标和滑动输入符号。长按两字母键弹出这两个字母，选中后先结束组字再上屏，与九键长按相同。拼音选择与九键共用左侧栏，空闲时显示符号，组字时纵向选择拼音，不挤占读音或候选行。
     */
    private void rebuildFourteenKeyRows() {
        s.imeLayoutRows.dismissNineKeyHoldOptions();
        boolean composing = s.hasEngineComposition();
        boolean eighteen = s.displayedTouchLayout(s.view) == KeyboardLayout.EIGHTEEN_KEY_LAYOUT;
        java.util.List<java.util.List<FourteenKeyLayout.Key>> rows = eighteen
            ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows();
        float letterWeight = eighteen ? 1f : FourteenKeyLayout.LETTER_WEIGHT;
        float edgeWeight = eighteen ? 1f : KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
        LinearLayout container = LayoutPolicy.row(s);
        s.imeStyler.adjustThreeRowBlockHeight(container);
        s.keyRows.addView(container, LayoutPolicy.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3)));
        container.addView(s.imeLayoutRows.fourteenSpellingSidebar(),
            LayoutPolicy.weightedMatchParentParams(0.7f));
        LinearLayout letterColumn = LayoutPolicy.column(s);
        container.addView(letterColumn, LayoutPolicy.weightedMatchParentParams(3.8f));
        for (int rowIndex = 0; rowIndex < rows.size(); rowIndex++) {
            LinearLayout row = LayoutPolicy.row(s);
            row.setTag(new MSIMEInputService.KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP,
                rows.size(), rowIndex, true));
            letterColumn.addView(row, LayoutPolicy.matchWidthWrapParams());
            boolean last = rowIndex == rows.size() - 1;
            if (eighteen && rowIndex == 1) {
                eighteenRowLeadingIndent = new View(s);
                row.addView(eighteenRowLeadingIndent, LayoutPolicy.weightedMatchParentParams(0.5f));
            }
            if (last) {
                Button separator = s.keyId(s.keyboardKey(FourteenKeyLayout.separatorFace(composing),
                    FourteenKeyLayout.separatorDescription(composing), this::fourteenKeySeparator),
                    "SoftSymbol");
                KeyboardTypographyPolicy.setKeyTextSize(separator, 15);
                if (separator instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
                fourteenSeparatorKey = separator;
                row.addView(separator, LayoutPolicy.weightedMatchParentParams(
                    edgeWeight));
            }
            for (FourteenKeyLayout.Key key : rows.get(rowIndex)) {
                Button keyButton = s.keyId(s.keyboardKey(FourteenKeyLayout.face(key),
                    FourteenKeyLayout.description(key), () -> s.gridKey(key.input())),
                    eighteen ? EighteenKeyLayout.keyId(key) : FourteenKeyLayout.keyId(key));
                keyButton.setText(FourteenKeyLayout.face(key, fourteenShuangpin()));
                keyButton.setContentDescription(FourteenKeyLayout.accessibilityLabel(key, fourteenShuangpin()));
                if ("l".equals(key.letters())) fourteenLKey = keyButton;
                if (eighteen) KeyboardTypographyPolicy.setFittedKeyTextSize(keyButton, 12, 22);
                else KeyboardTypographyPolicy.setKeyTextSize(keyButton, 22);
                if (keyButton instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
                String letters = FourteenKeyLayout.holdLetters(key);
                EighteenKeySwipePolicy.Gesture swipe = eighteen && letters.length() == 2
                    ? bindEighteenKeySwipe(keyButton, letters) : null;
                if (!letters.isEmpty()) {
                    keyButton.setOnLongClickListener(ignored -> {
                        if (swipe != null) swipe.hold();
                        // 长按是这个键的一次按压，之后从弹窗里选的不是另一次按键。
                        s.countKey(keyButton);
                        s.imeLayoutRows.showNineKeyHoldOptions(keyButton, "", letters);
                        return true;
                    });
                }
                row.addView(keyButton, LayoutPolicy.weightedMatchParentParams(letterWeight));
            }
            if (eighteen && rowIndex == 1) {
                eighteenRowTrailingIndent = new View(s);
                row.addView(eighteenRowTrailingIndent, LayoutPolicy.weightedMatchParentParams(0.5f));
            }
            if (last) addLetterRowEdgeKey(row, s.deleteButton, row.getChildCount(),
                edgeWeight);
        }
    }

    /** 十八键双拼的横滑进入当前网格；未接管时仍由 Button 处理点按和长按。 */
    private EighteenKeySwipePolicy.Gesture bindEighteenKeySwipe(Button key, String letters) {
        EighteenKeySwipePolicy.Gesture gesture = new EighteenKeySwipePolicy.Gesture(letters);
        String face = key.getText().toString();
        key.setOnTouchListener((view, event) -> {
            updateKeyPreview(key, event);
            if (!fourteenShuangpin()) {
                gesture.cancel();
                key.setText(face);
                hideKeyPreview(key);
                return false;
            }
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    s.imeLayoutRows.dismissNineKeyHoldOptions();
                    gesture.start(event.getPointerId(0), DimensionPolicy.fromPixels(s, event.getX()),
                        DimensionPolicy.fromPixels(s, event.getY()));
                    return false;
                }
                case MotionEvent.ACTION_MOVE, MotionEvent.ACTION_UP -> {
                    boolean wasConsumed = gesture.consumed();
                    gesture.move(event.getPointerId(0), event.getPointerCount(),
                        DimensionPolicy.fromPixels(s, event.getX()), DimensionPolicy.fromPixels(s, event.getY()));
                    if (!wasConsumed && gesture.consumed()) cancelKeyTouch(key, event);
                    char selected = gesture.selected();
                    String label = selected == 0 ? face : String.valueOf(selected);
                    key.setText(label);
                    updateKeyPreviewLabel(key, label);
                    if (event.getActionMasked() == MotionEvent.ACTION_UP) {
                        key.setText(face);
                        if (gesture.consumed()) key.setPressed(false);
                        if (selected != 0) {
                            s.imeKeyFeedback.playFeedback(key);
                            s.countKey(key);
                            s.gridKey(selected);
                        }
                    }
                    return gesture.consumed();
                }
                case MotionEvent.ACTION_POINTER_DOWN, MotionEvent.ACTION_POINTER_UP,
                        MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_OUTSIDE -> {
                    gesture.cancel();
                    cancelKeyTouch(key, event);
                    key.setText(face);
                    hideKeyPreview(key);
                    s.imeLayoutRows.dismissNineKeyHoldOptions();
                    return true;
                }
                default -> { return gesture.consumed(); }
            }
        });
        return gesture;
    }

    private void cancelKeyTouch(Button key, MotionEvent event) {
        MotionEvent cancel = MotionEvent.obtain(event);
        cancel.setAction(MotionEvent.ACTION_CANCEL);
        key.onTouchEvent(cancel);
        cancel.recycle();
        key.setPressed(false);
    }

    /** 两个半键占位合计保留一个键的横向边距，键距就地变化时也与上下排等宽。 */
    void applyEighteenKeyIndentGeometry(int halfGap) {
        if (eighteenRowLeadingIndent == null || eighteenRowTrailingIndent == null) return;
        LinearLayout.LayoutParams leading = (LinearLayout.LayoutParams) eighteenRowLeadingIndent.getLayoutParams();
        LinearLayout.LayoutParams trailing = (LinearLayout.LayoutParams) eighteenRowTrailingIndent.getLayoutParams();
        leading.setMargins(halfGap, 0, 0, 0);
        trailing.setMargins(0, 0, halfGap, 0);
        eighteenRowLeadingIndent.setLayoutParams(leading);
        eighteenRowTrailingIndent.setLayoutParams(trailing);
    }

    /** 分词键：组字时送 `'`，在已打的组码末尾定一个音节分界；空闲时打开符号面板。 */
    private void fourteenKeySeparator() {
        if (s.hasEngineComposition()) s.character('\'', false);
        else s.imePanels.showSymbolPanel();
    }

    private boolean fourteenShuangpin() {
        return InputViewValuePolicy.scheme(s.view, -1) == InputSchemeTraits.SHUANGPIN;
    }

    /** 14 键的分词键和分号入口跟着组字状态及方案更新，render 时调用。 */
    void updateFourteenKeySeparator() {
        if (fourteenLKey != null) {
            FourteenKeyLayout.Key key = new FourteenKeyLayout.Key("l");
            String face = FourteenKeyLayout.face(key, fourteenShuangpin());
            if (!face.contentEquals(fourteenLKey.getText())) {
                fourteenLKey.setText(face);
                fourteenLKey.setContentDescription(FourteenKeyLayout.accessibilityLabel(key, fourteenShuangpin()));
            }
        }
        if (fourteenSeparatorKey == null) return;
        boolean composing = s.hasEngineComposition();
        String face = FourteenKeyLayout.separatorFace(composing);
        if (face.contentEquals(fourteenSeparatorKey.getText())) return;
        fourteenSeparatorKey.setText(face);
        fourteenSeparatorKey.setContentDescription("按键 " + FourteenKeyLayout.separatorDescription(composing));
    }

    /**
     * 数字行（#6022）：本地设置「数字行」打开时画在字母上方的 1–0（{@link KeyboardLayout#NUMBER_ROW}）。高度和分摊键盘高度调整的方式都与第一行字母相同，所以键盘整体高出一行。数字键和字母一样经 {@link MSIMEInputService#type} 交给 Engine：没有组字时 Engine 不收、由宿主上屏这个数字；组字中当前页有对应候选的 1–9 由 runtime 选词，Engine 不收的 `0` 和没有候选时的 1–9 先按首选结束组合再上屏（{@link DeclinedKeyPolicy}）。分离式键盘时同样从中间分开。
     *
     * @param letterRows 字母行数，数字行按其中第一行的份额分摊高度调整
     */
    private void addNumberRow(int letterRows) {
        LinearLayout row = LayoutPolicy.row(s);
        row.setTag(new MSIMEInputService.KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP, letterRows, 0, true));
        s.keyRows.addView(row, LayoutPolicy.matchWidthWrapParams());
        for (String digit : KeyboardLayout.NUMBER_ROW) {
            char input = digit.charAt(0);
            Button key = s.keyboardKey(digit, digit, () -> s.type(input));
            KeyboardTypographyPolicy.setKeyTextSize(key, 20);
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
            s.keyId(key, KeyPressIds.forCharacter(input));
            row.addView(key, LayoutPolicy.weightedMatchParentParams(1));
        }
        if (builtSplit) splitRow(row);
    }

    /** 第五排与字母数字行使用同一高度角色，切层时保留行高及高度调整。 */
    private void addSymbolExtraRow() {
        builtLayerPunctuation = s.sendsChinesePunctuation();
        LinearLayout row = LayoutPolicy.row(s);
        row.setTag(new MSIMEInputService.KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP, 3, 0, true));
        s.keyRows.addView(row, LayoutPolicy.matchWidthWrapParams());
        for (String face : KeyboardLayout.symbolExtraRow(moreSymbols, numberPadDrawn(), builtLayerPunctuation)) {
            KeyboardLayout.LayerKey spec = new KeyboardLayout.LayerKey(face, face,
                KeyboardLayout.LayerKeyKind.CHARACTER, 1f);
            row.addView(designLayerKey(spec, false), LayoutPolicy.weightedMatchParentParams(1));
        }
        if (builtSplit) splitRow(row);
    }

    /** Re-parent a long-lived control into one end of the last letter row. */
    void addLetterRowEdgeKey(LinearLayout row, Button key, int index, float weight) {
        if (key == null) return;
        if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
        // ⇧ and ⌫ are function keys: the design tints them like 123 and 中.
        if (key instanceof KeyboardPressButton press)
            press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        s.imeStyler.styleButton(key, KeyboardKeyRole.ACCENT, s.skin);
        ViewPolicy.show(key);
        row.addView(key, index, LayoutPolicy.weightedMatchParentParams(weight));
    }

    /**
     * 新设计的 123 层（moreSymbols 为假）或 #+= 层：四行键自带底行（拼音 / ABC | 表情或符号 | 空格 | ↵），功能行这时整行不显示。字符键原样上屏，⌫ / 空格 / 回车用常驻的那几个键。标点用全角中文还是半角英文跟「中文标点」走（{@link MSIMEInputService#sendsChinesePunctuation}），不是跟中英文模式：0.2.0 起这里只看中英文模式，关掉中文标点后这一层照样上屏「，」「。」。
     */
    private void rebuildDesignLayer() {
        boolean chinese = !s.dedicatedEnglish;
        boolean chinesePunctuation = s.sendsChinesePunctuation();
        builtLayerPunctuation = chinesePunctuation;
        builtLayerToolbarEmoji = !s.toolbarHidden && s.toolbarEmoji;
        java.util.List<java.util.List<KeyboardLayout.LayerKey>> rows = moreSymbols
            ? KeyboardLayout.moreSymbolLayer(chinese, chinesePunctuation)
            : KeyboardLayout.numberLayer(chinese, chinesePunctuation, s.punctuationMarks().cornerQuotes(),
                builtLayerToolbarEmoji);
        for (int rowIndex = 0; rowIndex < rows.size(); rowIndex++) {
            LinearLayout row = LayoutPolicy.row(s);
            // 最后一行是这一层自带的底行，和功能行一样挂底行角色：和一行键同高，分到四行均分后底行的那一份调整；前三行和字母键一样分摊其余部分，整层与字母层同高。底行原先挂的是 rowCount 为 1 的普通角色，整份调整量在三行之外又加了一遍：调高时这一层比字母层高出一截，调到 75% 时底行被压成 0 高。
            boolean bottomRow = rowIndex == rows.size() - 1;
            row.setTag(bottomRow ? MSIMEInputService.KeyboardHeightRole.bottomRow()
                : new MSIMEInputService.KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP,
                    rows.size() - 1, rowIndex, true));
            s.keyRows.addView(row, bottomRow
                ? LayoutPolicy.matchWidthHeightPx(s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP))
                : LayoutPolicy.matchWidthWrapParams());
            for (KeyboardLayout.LayerKey layerKey : rows.get(rowIndex)) {
                Button key = designLayerKey(layerKey, rowIndex == 0);
                if (key == null) continue;
                if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
                ViewPolicy.show(key);
                row.addView(key, LayoutPolicy.weightedMatchParentParams(layerKey.weight()));
            }
            if (builtSplit) {
                if (rowIndex == rows.size() - 1) splitSpaceRow(row);
                else splitRow(row);
            }
        }
    }

    private Button designLayerKey(KeyboardLayout.LayerKey layerKey, boolean firstRow) {
        String text = layerKey.text();
        Button key;
        switch (layerKey.kind()) {
            case CHARACTER -> {
                key = s.keyboardKey(text, text, () -> s.imeLayoutRows.commitNineKeyLiteral(text));
                KeyboardTypographyPolicy.setKeyTextSize(key, firstRow ? 20 : 18);
                if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
                if (text.length() == 1) s.keyId(key, KeyPressIds.forCharacter(text.charAt(0)));
                bindSuperscriptShortcut(key, text);
                return key;
            }
            case LAYER_TOGGLE -> {
                key = s.keyboardKey(text, layerKey.description(), () -> {
                    moreSymbols = !moreSymbols;
                    rebuildKeyRows();
                    s.render();
                });
                s.keyId(key, "SoftLayer");
            }
            case LETTERS -> {
                key = s.keyboardKey(text, layerKey.description(), () -> {
                    moreSymbols = false;
                    s.keyboardLayer = KeyboardLayout.Layer.LETTERS;
                    rebuildKeyRows();
                    s.render();
                });
                s.keyId(key, "SoftLayer");
            }
            case EMOJI -> {
                KeyboardIconKey icon = new KeyboardIconKey(s, KeyboardIconKey.Kind.EMOJI);
                icon.setText(text);
                s.bindCountedAction(icon, () -> {
                    s.imePanels.showEmojiPicker();
                    // 底行入口也刷新工具栏的返回动作；隐藏工具栏时，面板需要临时显示它。
                    s.render();
                });
                key = icon;
            }
            case PUNCTUATION_TOGGLE -> {
                key = s.keyboardKey(text, layerKey.description(), s::toggleChinesePunctuation);
                s.keyId(key, "SoftPunctuation");
            }
            case SYMBOL_PANEL -> {
                key = s.keyboardKey(text, layerKey.description(), () -> {
                    s.imePanels.showSymbolPanel();
                    s.render();
                });
                s.keyId(key, "SoftSymbol");
            }
            case DELETE -> {
                key = s.deleteButton;
                if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
                if (key != null) s.imeStyler.styleButton(key, KeyboardKeyRole.ACCENT, s.skin);
                return key;
            }
            case SPACE -> {
                key = s.spaceButton;
                if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
                if (key != null) s.imeStyler.styleButton(key, KeyboardKeyRole.KEY, s.skin);
                return key;
            }
            case RETURN -> {
                key = s.enterButton;
                if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.RETURN);
                if (key != null) s.imeStyler.styleButton(key, KeyboardKeyRole.RETURN, s.skin);
                return key;
            }
            default -> { return null; }
        }
        // 功能键（层切换、返回字母、表情、符号）：功能键底色、15 sp，描述按 §2.8。
        key.setContentDescription(layerKey.description());
        KeyboardTypographyPolicy.setKeyTextSize(key, 15);
        if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        s.imeStyler.styleButton(key, KeyboardKeyRole.ACCENT, s.skin);
        return key;
    }

    /** 九键主键面与 123 / #+= 层的叹号：上滑输入 0，长按选择上标或下标 0；轻点仍输入叹号。 */
    void bindSuperscriptShortcut(Button key, String text) {
        if (!SuperscriptShortcutPolicy.applies(text)) return;
        key.setContentDescription(text + "；上滑输入 0；长按选择上标 ⁰ 或下标 ₀");
        key.setOnLongClickListener(ignored -> {
            s.imeLayoutRows.showSuperscriptOptions(key);
            return true;
        });
        final float[] downY = new float[1];
        final boolean[] swiped = new boolean[1];
        key.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case android.view.MotionEvent.ACTION_DOWN -> {
                    downY[0] = DimensionPolicy.fromPixels(s, event.getY());
                    swiped[0] = false;
                    return false;
                }
                case android.view.MotionEvent.ACTION_MOVE -> {
                    if (!swiped[0] && SuperscriptShortcutPolicy.swipedUp(downY[0],
                            DimensionPolicy.fromPixels(s, event.getY()))) {
                        swiped[0] = true;
                        key.cancelLongPress();
                        android.view.MotionEvent cancel = android.view.MotionEvent.obtain(event);
                        cancel.setAction(android.view.MotionEvent.ACTION_CANCEL);
                        key.onTouchEvent(cancel);
                        cancel.recycle();
                    }
                    return swiped[0];
                }
                case android.view.MotionEvent.ACTION_UP -> {
                    if (!swiped[0]) return false;
                    swiped[0] = false;
                    key.setPressed(false);
                    s.imeKeyFeedback.playFeedback(key);
                    s.countKey(key);
                    s.imeLayoutRows.commitNineKeyLiteral(SuperscriptShortcutPolicy.ZERO);
                    return true;
                }
                case android.view.MotionEvent.ACTION_CANCEL -> {
                    swiped[0] = false;
                    key.setPressed(false);
                    return false;
                }
                default -> { return swiped[0]; }
            }
        });
    }
}
