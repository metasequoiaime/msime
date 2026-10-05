package app.msime.android;

import android.graphics.Color;
import android.util.TypedValue;
import android.view.MotionEvent;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;

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
    /** 气泡当前跟随的键；多指交替时只有它的松手才收起气泡。 */
    private Button keyPreviewOwner;
    /** 符号层里正在显示 #+= 页（否则是 123 页）。 */
    private boolean moreSymbols;
    /** 第二行（a–l）两侧各 5% 的缩进占位；微软双拼的第十个键出现时收起。 */
    private View secondRowLeadingIndent;
    private View secondRowTrailingIndent;

    /** 新设计的 123 / #+= 层画在哪些界面上：26 键（含韩文键面）以及把符号页交给 26 键行的手写和笔画；注音的数字键另有用途，保留原符号行。 */
    static boolean drawsDesignLayer(int touchLayout) {
        return touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
            || touchLayout == KeyboardLayout.KOREAN_LAYOUT
            || touchLayout == KeyboardLayout.HANDWRITING_LAYOUT
            || touchLayout == KeyboardLayout.STROKE_LAYOUT;
    }

    /** 偏好里的一个布尔触控开关。 */
    private boolean touchPreference(String key) {
        return s.imeBottomRow.touchPreference(key);
    }

    /** 把 SVC 建的 ⇧ ⌫ 换成画图标的键（每次 onCreateInputView 新建控件后各换一次）。 */
    void ensureIconKeys() {
        if (s.shiftButton != null && !(s.shiftButton instanceof KeyboardIconKey)) {
            Button original = s.shiftButton;
            KeyboardIconKey key = new KeyboardIconKey(s, KeyboardIconKey.Kind.SHIFT);
            key.setText(original.getText());
            key.setContentDescription(original.getContentDescription());
            key.setOnClickListener(ignored -> original.performClick());
            s.keyId(key, "ShiftLeft");
            s.shiftButton = key;
        }
        if (s.deleteButton != null && !(s.deleteButton instanceof KeyboardIconKey)) {
            Button original = s.deleteButton;
            KeyboardIconKey key = new KeyboardIconKey(s, KeyboardIconKey.Kind.BACKSPACE);
            key.setText("⌫");
            key.setContentDescription("删除");
            key.setOnClickListener(ignored -> original.performClick());
            s.keyId(key, "Backspace");
            bindBackspaceRepeat(key, s::deleteFromHandwriting);
            s.deleteButton = key;
        }
    }

    /** 按键气泡；第一次用时加进覆盖层。 */
    private KeyboardKeyPreview keyPreview() {
        if (keyPreviewLayer == null) return null;
        if (keyPreview == null || keyPreview.getParent() != keyPreviewLayer) {
            keyPreview = new KeyboardKeyPreview(s);
            keyPreviewLayer.addView(keyPreview, new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT));
        }
        return keyPreview;
    }

    private void showKeyPreview(Button key, String label) {
        // No enlarged bubble in password fields: it would show each typed character to anyone looking at the screen.
        if (EditorPolicy.password(s.editorInputType)) return;
        KeyboardKeyPreview preview = keyPreview();
        if (preview == null || !key.isAttachedToWindow()) return;
        KeyboardSkin skin = s.imeStyler.themed(s.skin);
        preview.setColors(Color.parseColor(skin.keyBackground()),
            Color.parseColor(skin.keyForeground()), Color.argb(20, 0, 0, 0));
        int[] keyAt = new int[2];
        int[] layerAt = new int[2];
        key.getLocationInWindow(keyAt);
        keyPreviewLayer.getLocationInWindow(layerAt);
        float weight = key.getLayoutParams() instanceof LinearLayout.LayoutParams params
            ? params.weight : 1f;
        preview.show(label, keyAt[0] - layerAt[0], keyAt[1] - layerAt[1], key.getWidth(), weight,
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

    /**
     * 字母键的按压：开着「按键弹出」（`touch_key_popup`）时浮出气泡；开着「下滑输入符号」（`touch_swipe_down_symbols`）时，下滑超过 14 dp 松手输入右上角的提示字符，气泡同时改显示它。没有下滑时照常交给按钮自己的点击。
     */
    private void bindLetterGestures(Button key, String face, String hint) {
        final float[] downY = new float[1];
        final boolean[] swiped = new boolean[1];
        final float density = s.getResources().getDisplayMetrics().density;
        key.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    downY[0] = event.getY() / density;
                    swiped[0] = false;
                    if (touchPreference(AndroidLocalSettings.KEY_POPUP)) showKeyPreview(key, face);
                    return false;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (hint != null && !swiped[0] && touchPreference(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS)
                            && SwipeDownHintPolicy.swiped(downY[0], event.getY() / density)) {
                        swiped[0] = true;
                        if (keyPreview != null && keyPreviewOwner == key) keyPreview.setLabel(hint);
                    }
                    return swiped[0];
                }
                case MotionEvent.ACTION_UP -> {
                    hideKeyPreview(key);
                    if (!swiped[0]) return false;
                    MotionEvent cancel = MotionEvent.obtain(event);
                    cancel.setAction(MotionEvent.ACTION_CANCEL);
                    key.onTouchEvent(cancel);
                    cancel.recycle();
                    key.setPressed(false);
                    s.imeKeyFeedback.playFeedback(key);
                    s.countKey(key);
                    s.imeLayoutRows.commitNineKeyLiteral(hint);
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    hideKeyPreview(key);
                    return false;
                }
                default -> { return false; }
            }
        });
    }

    /** 第二行的 5% 缩进只在 9 键行出现；微软双拼的第十个键可见时收起（每次渲染校正一次）。 */
    void updateSecondRowIndent() {
        if (secondRowLeadingIndent == null) return;
        String localMode = s.view == null ? "none" : s.view.optString("local_mode", "none");
        boolean tenKeys = s.microsoftFinalKey != null && MicrosoftShuangpinKeyPolicy.visible(
            s.dedicatedEnglish, s.selectedScheme, localMode);
        int visibility = tenKeys ? View.GONE : View.VISIBLE;
        if (secondRowLeadingIndent.getVisibility() != visibility) {
            secondRowLeadingIndent.setVisibility(visibility);
            secondRowTrailingIndent.setVisibility(visibility);
        }
    }

    private View indent() {
        View spacer = new View(s);
        spacer.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        return spacer;
    }

    /** Matches the Apple delete key: a short tap deletes once, a held press repeats. */
    void bindBackspaceRepeat(Button button, Runnable action) {
        button.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    cancelBackspaceRepeat();
                    s.backspaceRepeatButton = button;
                    s.backspaceRepeated = false;
                    s.backspaceClearedComposition = false;
                    button.setPressed(true);
                    s.backspaceRepeatTask = new Runnable() {
                        @Override public void run() {
                            if (s.backspaceRepeatButton != button || !button.isPressed()) return;
                            // A held delete is one press however often it repeats; a short tap counts through performClick instead.
                            if (!s.backspaceRepeated) s.countKey(button);
                            s.backspaceRepeated = true;
                            if (s.hasEngineComposition()) {
                                s.imeKeyFeedback.playFeedback(button);
                                // A held delete drops the whole syllable, also through an open Hanja list.
                                s.discardComposition();
                                s.backspaceClearedComposition = true;
                                s.main.removeCallbacks(this);
                                s.backspaceRepeatTask = null;
                                return;
                            }
                            s.imeKeyFeedback.playFeedback(button);
                            action.run();
                            s.main.postDelayed(this, BackspaceRepeatPolicy.REPEAT_INTERVAL_MS);
                        }
                    };
                    s.main.postDelayed(s.backspaceRepeatTask, BackspaceRepeatPolicy.INITIAL_DELAY_MS);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (event.getX() < 0 || event.getY() < 0
                            || event.getX() >= button.getWidth()
                            || event.getY() >= button.getHeight()) {
                        cancelBackspaceRepeat();
                        button.setPressed(false);
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    boolean active = s.backspaceRepeatButton == button;
                    boolean repeated = active && (s.backspaceRepeated || s.backspaceClearedComposition);
                    cancelBackspaceRepeat();
                    button.setPressed(false);
                    if (active && !repeated) button.performClick();
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

    void cancelBackspaceRepeat() {
        if (s.backspaceRepeatTask != null) s.main.removeCallbacks(s.backspaceRepeatTask);
        s.backspaceRepeatTask = null;
        if (s.backspaceRepeatButton != null) s.backspaceRepeatButton.setPressed(false);
        s.backspaceRepeatButton = null;
        s.backspaceRepeated = false;
        s.backspaceClearedComposition = false;
    }

    void rebuildKeyRows() {
        if (s.keyRows == null) return;
        ensureIconKeys();
        s.imeBottomRow.ensureFaces();
        hideKeyPreview();
        secondRowLeadingIndent = null;
        secondRowTrailingIndent = null;
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS) moreSymbols = false;
        s.imeLayoutRows.hideJapaneseFlickPreview();
        s.deactivateHandwriting();
        s.symbolKeyButtons.clear();
        s.symbolKeyInputs.clear();
        s.shuangpinKeyButtons.clear();
        s.shuangpinKeyInputs.clear();
        s.microsoftFinalKey = null;
        s.nineKeySidebar = null;
        s.strokeWildcardKey = null;
        s.japaneseSpaceKey = null;
        s.japaneseReturnKey = null;
        s.japaneseSymbolsKey = null;
        s.japaneseVariantsButton = null;
        s.keyRows.removeAllViews();
        if (s.displayedTouchLayout(s.view) == MSIMEInputService.JAPANESE_NINE_KEY_LAYOUT) {
            s.imeLayoutRows.rebuildJapaneseNineKeyRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        // 九键切数字仍然是九键。The digit layer keeps the grid the user picked three columns for;
        // only handwriting hands its panel over to the 26-key symbol rows.
        if (s.displayedTouchLayout(s.view) == MSIMEInputService.QUANPIN_NINE_KEY_LAYOUT) {
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
        // 笔画键盘的符号页与手写一样交给 26 键符号行，字母层才画笔画网格。
        if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.STROKE_LAYOUT) {
            s.imeLayoutRows.rebuildStrokeRows();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        if (s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                && drawsDesignLayer(s.displayedTouchLayout(s.view))) {
            rebuildDesignLayer();
            s.imeBottomRow.updateActionRow();
            s.imeStyler.applyKeyboardGeometry();
            return;
        }
        // 越南语字母和藏文的威利转写字母按敲下的大小写写入，所以键面像英文键一样显示大小写，而不是中文键盘的大写键面。
        boolean chineseMode = !s.dedicatedEnglish && !s.letterCaseSchemeActive();
        boolean localMode = s.view != null
            && !"none".equals(s.view.optString("local_mode", "none"));
        boolean shifted = s.letterCase.usesUppercase();
        boolean koreanKeycaps = s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.KOREAN_LAYOUT;
        boolean zhuyinLayout = s.displayedTouchLayout(s.view) == KeyboardLayout.ZHUYIN_LAYOUT;
        boolean zhuyinKeycaps = zhuyinLayout && s.keyboardLayer == KeyboardLayout.Layer.LETTERS;
        // 新设计的字母键：22 sp 键面，26 键（不含韩文与注音键面）右上角画下滑提示符。
        boolean standardLetters = s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && !koreanKeycaps && !zhuyinKeycaps;
        boolean cornerHints = standardLetters && touchPreference(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS);
        // The face is the policy's job; the key itself always sends its canonical lowercase form.
        java.util.List<java.util.List<String>> rows = KeyboardLayout.rows(s.keyboardLayer,
            s.displayedTouchLayout(s.view));
        for (int rowIndex = 0; rowIndex < rows.size(); rowIndex++) {
            java.util.List<String> keys = rows.get(rowIndex);
            LinearLayout row = new LinearLayout(s);
            row.setTag(new MSIMEInputService.KeyboardHeightRole(KeyboardGeometry.STANDARD_ROW_HEIGHT_DP,
                rows.size(), rowIndex, true));
            s.keyRows.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            boolean tibetanSymbols = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                && s.tibetanSchemeActive();
            // 第二行（a–l）两侧各缩进 5%：9 个键加两侧各 0.5 的占位正好是第一行 10 个键的宽度。
            if (standardLetters && rowIndex == 1) {
                secondRowLeadingIndent = indent();
                row.addView(secondRowLeadingIndent, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, .5f));
            }
            for (String rowKey : keys) {
                // 藏文的符号页把 `=` 换成叠写用的 `+`。
                final String key = KeyboardLayout.symbolRowKey(rowKey, tibetanSymbols);
                final String input = key;
                String face = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                    ? ChineseSymbolFaces.face(key, s.sendsChinesePunctuation())
                    : koreanKeycaps ? KoreanKeyboardLayout.face(key, shifted)
                    : zhuyinKeycaps ? ZhuyinKeyboardLayout.face(key)
                    : LetterKeyFacePolicy.face(key, chineseMode, localMode, shifted);
                Button keyButton;
                if (zhuyinKeycaps) {
                    // Bopomofo keycaps over the Dachen keys: type() sends the ASCII key, and the Engine spells and converts.
                    keyButton = s.keyboardKey(face, face, () -> s.type(input.charAt(0)));
                    keyButton.setContentDescription(ZhuyinKeyboardLayout.accessibilityLabel(key));
                    if (keyButton instanceof KeyboardPressButton press)
                        press.setKeyboardRole(KeyboardKeyRole.KEY);
                } else if (zhuyinLayout && ZhuyinKeyboardLayout.claimsSymbol(key)) {
                    // Dachen reads these digits and marks as bopomofo and tone keys, so the symbol page writes what its key shows instead of handing the key to the Engine.
                    keyButton = s.keyboardKey(face, face, () -> s.imeLayoutRows.commitNineKeyLiteral(
                        ChineseSymbolFaces.face(input, s.sendsChinesePunctuation())));
                } else if (koreanKeycaps) {
                    // Jamo keycaps over the same QWERTY letters: type() sends the letter, and the Engine composes the syllable.
                    keyButton = s.keyboardKey(face, face, () -> s.type(input.charAt(0)));
                    keyButton.setContentDescription(
                        KoreanKeyboardLayout.accessibilityLabel(key, shifted));
                    if (keyButton instanceof KeyboardPressButton press)
                        press.setKeyboardRole(KeyboardKeyRole.KEY);
                } else if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS) {
                    ShuangpinHintButton hintButton = s.shuangpinKeyboardKey(
                        face, face, () -> s.type(input.charAt(0)));
                    keyButton = hintButton;
                    s.shuangpinKeyButtons.add(hintButton);
                    s.shuangpinKeyInputs.add(input);
                    String hint = cornerHints ? LetterHintTable.hint(input) : null;
                    hintButton.setCornerHint(hint);
                    hintButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 22);
                    bindLetterGestures(hintButton, face, hint);
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
                row.addView(keyButton, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, 1));
            }
            // The Dachen rows carry their own ; key (ㄤ), and no double-pinyin final.
            if (s.keyboardLayer == KeyboardLayout.Layer.LETTERS && rowIndex == 1 && !zhuyinKeycaps) {
                s.microsoftFinalKey = s.keyId(s.shuangpinKeyboardKey(";", "微软双拼 ing", () -> s.type(';')),
                    "Semicolon");
                s.shuangpinKeyButtons.add((ShuangpinHintButton) s.microsoftFinalKey);
                s.shuangpinKeyInputs.add(";");
                s.microsoftFinalKey.setTextSize(TypedValue.COMPLEX_UNIT_SP, 22);
                row.addView(s.microsoftFinalKey, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, 1));
            }
            if (standardLetters && rowIndex == 1) {
                secondRowTrailingIndent = indent();
                row.addView(secondRowTrailingIndent, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, .5f));
                updateSecondRowIndent();
            }
            // 大小写和删除属于最后一行的两端，不属于底部功能行。Leaving them in a strip below the keys
            // is what pushed every other control out of reach of a thumb.
            if (rowIndex == rows.size() - 1) {
                int layout = s.displayedTouchLayout(s.view);
                boolean symbols = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
                float edge = symbols ? KeyboardActionRow.letterRowEdgeWeight(true)
                    : KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
                if (KeyboardActionRow.rowsCarryCase(layout, symbols))
                    addLetterRowEdgeKey(row, s.shiftButton, 0, edge);
                if (KeyboardActionRow.rowsCarryDelete(layout, symbols))
                    addLetterRowEdgeKey(row, s.deleteButton, row.getChildCount(), edge);
            }
        }
        s.imeStyler.applyKeyboardGeometry();
    }

    /** Re-parent a long-lived control into one end of the last letter row. */
    void addLetterRowEdgeKey(LinearLayout row, Button key, int index, float weight) {
        if (key == null) return;
        if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
        // ⇧ and ⌫ are function keys: the design tints them like 123 and 中.
        if (key instanceof KeyboardPressButton press)
            press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        s.imeStyler.styleButton(key, KeyboardKeyRole.ACCENT, s.skin);
        key.setVisibility(View.VISIBLE);
        row.addView(key, index, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, weight));
    }

    /**
     * 新设计的 123 层（moreSymbols 为假）或 #+= 层：四行键自带底行（拼音 / ABC | 表情或符号 | 空格 | ↵），功能行这时整行不显示。字符键原样上屏（中文模式的全角符号也是），⌫ / 空格 / 回车用常驻的那几个键。
     */
    private void rebuildDesignLayer() {
        boolean chinese = !s.dedicatedEnglish;
        java.util.List<java.util.List<KeyboardLayout.LayerKey>> rows = moreSymbols
            ? KeyboardLayout.moreSymbolLayer(chinese) : KeyboardLayout.numberLayer(chinese);
        for (int rowIndex = 0; rowIndex < rows.size(); rowIndex++) {
            LinearLayout row = new LinearLayout(s);
            row.setTag(new MSIMEInputService.KeyboardHeightRole(KeyboardGeometry.STANDARD_ROW_HEIGHT_DP,
                rows.size(), rowIndex, true));
            s.keyRows.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            for (KeyboardLayout.LayerKey layerKey : rows.get(rowIndex)) {
                Button key = designLayerKey(layerKey, rowIndex == 0);
                if (key == null) continue;
                if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
                key.setVisibility(View.VISIBLE);
                row.addView(key, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, layerKey.weight()));
            }
        }
    }

    private Button designLayerKey(KeyboardLayout.LayerKey layerKey, boolean firstRow) {
        String text = layerKey.text();
        Button key;
        switch (layerKey.kind()) {
            case CHARACTER -> {
                key = s.keyboardKey(text, text, () -> s.imeLayoutRows.commitNineKeyLiteral(text));
                key.setTextSize(TypedValue.COMPLEX_UNIT_SP, firstRow ? 20 : 18);
                if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
                if (text.length() == 1) s.keyId(key, KeyPressIds.forCharacter(text.charAt(0)));
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
                icon.setOnClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(icon);
                    s.countKey(icon);
                    s.imePanels.showEmojiPicker();
                });
                key = icon;
            }
            case SYMBOL_PANEL -> {
                key = s.keyboardKey(text, layerKey.description(), s.imePanels::showSymbolPanel);
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
        key.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        s.imeStyler.styleButton(key, KeyboardKeyRole.ACCENT, s.skin);
        return key;
    }
}
