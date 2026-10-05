package app.msime.android;

import android.view.MotionEvent;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;

/** 字母键区（26 键与符号层）的行、删除键连删，以及给按键气泡预留的覆盖层；从 MSIMEInputService 原样搬出。 */
final class ImeLetterRows {
    private final MSIMEInputService s;

    ImeLetterRows(MSIMEInputService s) {
        this.s = s;
    }

    /** 按键气泡的覆盖层：onCreateInputView 建好后盖在整个键盘上，初始为空。 */
    FrameLayout keyPreviewLayer;

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
                            s.main.postDelayed(this, MSIMEInputService.BACKSPACE_REPEAT_INTERVAL_MILLIS);
                        }
                    };
                    s.main.postDelayed(s.backspaceRepeatTask, MSIMEInputService.BACKSPACE_REPEAT_DELAY_MILLIS);
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
        // 越南语字母和藏文的威利转写字母按敲下的大小写写入，所以键面像英文键一样显示大小写，而不是中文键盘的大写键面。
        boolean chineseMode = !s.dedicatedEnglish && !s.letterCaseSchemeActive();
        boolean localMode = s.view != null
            && !"none".equals(s.view.optString("local_mode", "none"));
        boolean shifted = s.letterCase.usesUppercase();
        boolean koreanKeycaps = s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.KOREAN_LAYOUT;
        boolean zhuyinLayout = s.displayedTouchLayout(s.view) == KeyboardLayout.ZHUYIN_LAYOUT;
        boolean zhuyinKeycaps = zhuyinLayout && s.keyboardLayer == KeyboardLayout.Layer.LETTERS;
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
                row.addView(s.microsoftFinalKey, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, 1));
            }
            // 大小写和删除属于最后一行的两端，不属于底部功能行。Leaving them in a strip below the keys
            // is what pushed every other control out of reach of a thumb.
            if (rowIndex == rows.size() - 1) {
                int layout = s.displayedTouchLayout(s.view);
                boolean symbols = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
                float edge = KeyboardActionRow.letterRowEdgeWeight(symbols);
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
        key.setVisibility(View.VISIBLE);
        row.addView(key, index, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, weight));
    }
}
