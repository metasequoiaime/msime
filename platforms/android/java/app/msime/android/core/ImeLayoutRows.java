package app.msime.android;

import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.PopupWindow;
import android.widget.TextView;
import org.json.JSONArray;
import org.json.JSONException;

/** 26 键以外的键区：手写、九键、笔画、日语九键（含 flick 与长按选项）、侧栏与九键拼音选择；从 MSIMEInputService 原样搬出。 */
final class ImeLayoutRows {
    private final MSIMEInputService s;

    ImeLayoutRows(MSIMEInputService s) {
        this.s = s;
    }

    void rebuildHandwritingRows() {
        s.handwritingStatus = new TextView(s);
        s.handwritingStatus.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        s.handwritingStatus.setGravity(Gravity.CENTER);
        s.handwritingStatus.setText("在此手写，停笔后选字");
        s.handwritingStatus.setContentDescription("手写状态");
        s.handwritingStatus.setClickable(false);
        s.handwritingStatus.setFocusable(false);

        FrameLayout row = new FrameLayout(s);
        s.handwritingCanvas = new HandwritingCanvas(s);
        s.handwritingCanvas.applySkin(s.skin);
        row.addView(s.handwritingCanvas, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));

        FrameLayout cardFrame = new FrameLayout(s);
        cardFrame.setClickable(false);
        cardFrame.setFocusable(false);
        FrameLayout.LayoutParams cardParams = new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT);
        cardParams.rightMargin = s.pixels(64);
        row.addView(cardFrame, cardParams);
        cardFrame.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> s.handwritingCanvas.setCardRect(
                    left, top, right, bottom));
        cardFrame.addView(s.handwritingStatus, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT,
            Gravity.CENTER));

        s.handwritingDownload = new Button(s);
        s.handwritingDownload.setAllCaps(false);
        s.handwritingDownload.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.handwritingDownload);
            s.downloadHandwritingModel();
        });
        FrameLayout.LayoutParams downloadParams = new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, s.pixels(48));
        downloadParams.gravity = Gravity.CENTER;
        downloadParams.leftMargin = s.pixels(16);
        downloadParams.rightMargin = s.pixels(16);
        cardFrame.addView(s.handwritingDownload, downloadParams);

        LinearLayout tools = new LinearLayout(s);
        tools.setOrientation(LinearLayout.VERTICAL);
        addNineKey(tools, s.keyboardKey("撤销", "撤销最后一笔", () -> {
            if (s.handwritingCanvas != null) s.handwritingCanvas.undo();
        }));
        addNineKey(tools, s.keyboardKey("清空", "清空手写", s::clearHandwriting));
        addNineKey(tools, s.keyId(s.keyboardKey("⌫", "删除", s::deleteFromHandwriting), "Backspace"));
        row.addView(tools, new FrameLayout.LayoutParams(s.pixels(64),
            FrameLayout.LayoutParams.MATCH_PARENT, Gravity.END));
        s.imeStyler.adjustFixedHeight(row, KeyboardGeometry.HANDWRITING_BODY_HEIGHT_DP);
        s.keyRows.addView(row);

        s.handwritingRecognizer = HandwritingRecognizerFactory.create(s);
        s.handwritingCanvas.setListener(new HandwritingCanvas.Listener() {
            @Override public void onStrokeBegan() {
                s.invalidateHandwritingRecognition();
                s.showHandwritingStatus("书写中…");
            }

            @Override public void onInkChanged(long revision,
                    java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
                s.handwritingInkChanged(revision, strokes);
            }
        });
        s.refreshHandwritingAvailability();
    }

    void addNineKey(LinearLayout parent, Button key) {
        boolean horizontal = parent.getOrientation() == LinearLayout.HORIZONTAL;
        parent.addView(key, new LinearLayout.LayoutParams(
            horizontal ? 0 : LinearLayout.LayoutParams.MATCH_PARENT,
            horizontal ? LinearLayout.LayoutParams.MATCH_PARENT : 0, 1));
    }

    /** The rail behind the capless punctuation column; it is not a Button, so the skin pass misses it. */
    void applySidebarRail() {
        if (s.nineKeySidebar == null) return;
        GradientDrawable rail = new GradientDrawable();
        rail.setColor(Color.parseColor(s.skin.sidebarBackground()));
        rail.setCornerRadius(s.pixels(s.skin.cornerRadius()));
        s.nineKeySidebar.setBackground(rail);
    }

    void rebuildNineKeyRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = new LinearLayout(s);
        container.setOrientation(LinearLayout.HORIZONTAL);
        s.imeStyler.adjustFixedHeight(container, KeyboardGeometry.NINE_KEY_HEIGHT_DP);
        s.keyRows.addView(container, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(180)));

        LinearLayout punctuation = new LinearLayout(s);
        punctuation.setOrientation(LinearLayout.VERTICAL);
        for (String symbol : NineKeyLayout.punctuation()) {
            Button key = s.keyId(s.keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            // The four punctuation keys share one rail rather than wearing four caps of their own.
            if (key instanceof KeyboardPressButton press)
                press.setKeyboardRole(KeyboardKeyRole.PLAIN);
            punctuation.addView(key, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        FrameLayout sidebar = new FrameLayout(s);
        s.nineKeySidebar = sidebar;
        applySidebarRail();
        sidebar.addView(punctuation, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        if (s.nineKeySpellingScroll != null) {
            // 拼音选择条只在创建键盘视图时建一次，每次重建九键都会换一个新的侧栏；偏好变化触发第二次重建时它还挂在上一个侧栏上，不先摘下来，addView 会抛 IllegalStateException 让键盘进程崩溃。
            if (s.nineKeySpellingScroll.getParent() instanceof android.view.ViewGroup previous)
                previous.removeView(s.nineKeySpellingScroll);
            sidebar.addView(s.nineKeySpellingScroll, new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        }
        container.addView(sidebar, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.7f));

        LinearLayout grid = new LinearLayout(s);
        grid.setOrientation(LinearLayout.VERTICAL);
        boolean digits = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
        for (java.util.List<NineKeyLayout.Key> keys : NineKeyLayout.rows()) {
            LinearLayout row = new LinearLayout(s);
            for (NineKeyLayout.Key key : keys) {
                String description = NineKeyLayout.description(key, digits);
                // On the digit layer the grid is a numeric keypad, so a tap commits the number
                // instead of feeding it to the pinyin session.
                NineKeyDigitButton keyButton = s.nineKeyGridKey(
                    NineKeyLayout.face(key, digits), description,
                    digits ? () -> commitNineKeyLiteral(NineKeyLayout.digitInput(key))
                        : () -> s.character(key.input()));
                s.keyId(keyButton, KeyPressIds.forNineKeyDigit(key.digit()));
                // 字母键面上印着它送进引擎的数字；数字键面本身就是那个数字，不必再印一次。
                // 分词键送的是拼音分隔符而不是 1，所以它没有可印的数字。
                keyButton.setDigitText(digits || !Character.isDigit(key.input())
                    ? "" : NineKeyLayout.digitInput(key));
                if (!digits && Character.isDigit(key.input()) && key.label().length() > 1) {
                    keyButton.setContentDescription("按键 " + description + "；长按输入数字或字母");
                    keyButton.setOnLongClickListener(ignored -> {
                        // The hold is this cell's press; picking from the popup is not another key.
                        s.countKey(keyButton);
                        showNineKeyHoldOptions(keyButton, key);
                        return true;
                    });
                }
                addNineKey(row, keyButton);
            }
            grid.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        container.addView(grid, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 3));

        LinearLayout actions = new LinearLayout(s);
        actions.setOrientation(LinearLayout.VERTICAL);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.keyboardKey("⌫", "删除", deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        addNineKey(actions, delete);
        addNineKey(actions, s.keyId(s.keyboardKey(".", "句点", this::commitNineKeyPeriod), "Period"));
        addNineKey(actions, s.keyId(s.keyboardKey("0", "数字 0", () -> commitNineKeyLiteral("0")),
            "Nine0"));
        container.addView(actions, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.8f));
    }

    /**
     * 笔画键盘：九键外框（标点侧栏、⌫ 列、固定高度）中间换成 {@link StrokeKeyboardLayout} 的 2×3 笔画网格。
     *
     * <p>笔画键直接走 character()，不走 type()：type() 会套用 Shift 大小写，也会把 ASCII 标点交给标点路径。九键的拼音选择条只属于拼音九键，这里不挂（挂上去要先从旧侧栏摘下，否则 addView 会抛异常）。
     */
    void rebuildStrokeRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = new LinearLayout(s);
        container.setOrientation(LinearLayout.HORIZONTAL);
        s.imeStyler.adjustFixedHeight(container, KeyboardGeometry.NINE_KEY_HEIGHT_DP);
        s.keyRows.addView(container, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(180)));

        LinearLayout punctuation = new LinearLayout(s);
        punctuation.setOrientation(LinearLayout.VERTICAL);
        for (String symbol : NineKeyLayout.punctuation()) {
            Button key = s.keyId(s.keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            if (key instanceof KeyboardPressButton press)
                press.setKeyboardRole(KeyboardKeyRole.PLAIN);
            punctuation.addView(key, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        FrameLayout sidebar = new FrameLayout(s);
        s.nineKeySidebar = sidebar;
        applySidebarRail();
        sidebar.addView(punctuation, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        container.addView(sidebar, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.7f));

        LinearLayout grid = new LinearLayout(s);
        grid.setOrientation(LinearLayout.VERTICAL);
        for (java.util.List<StrokeKeyboardLayout.Key> keys : StrokeKeyboardLayout.rows()) {
            LinearLayout row = new LinearLayout(s);
            for (StrokeKeyboardLayout.Key key : keys) {
                Button keyButton = s.keyboardKey(StrokeKeyboardLayout.face(key),
                    StrokeKeyboardLayout.accessibilityLabel(key), () -> strokeKey(key));
                keyButton.setContentDescription(StrokeKeyboardLayout.accessibilityLabel(key));
                if (keyButton instanceof KeyboardPressButton press)
                    press.setKeyboardRole(KeyboardKeyRole.KEY);
                s.keyId(keyButton, KeyPressIds.forCharacter(key.input()));
                if (key.input() == StrokeKeyboardLayout.WILDCARD) s.strokeWildcardKey = keyButton;
                addNineKey(row, keyButton);
            }
            grid.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        container.addView(grid, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 3));

        LinearLayout actions = new LinearLayout(s);
        actions.setOrientation(LinearLayout.VERTICAL);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.connection.deleteSurroundingTextInCodePoints(1, 0);
        };
        Button delete = s.keyId(s.keyboardKey("⌫", "删除", deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        addNineKey(actions, delete);
        addNineKey(actions, s.keyId(s.keyboardKey(".", "句点", this::commitNineKeyPeriod), "Period"));
        addNineKey(actions, s.keyId(s.keyboardKey("0", "数字 0", () -> commitNineKeyLiteral("0")),
            "Nine0"));
        container.addView(actions, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.8f));
        updateStrokeWildcardKey();
    }

    /** 笔画键送出它的字母；Engine 不收的键（空组合时的通配）什么也不写，免得往输入框里漏一个 x。 */
    void strokeKey(StrokeKeyboardLayout.Key key) {
        if (s.connection == null) return;
        if (!StrokeKeyboardLayout.sends(key.input(), s.hasEngineComposition())) return;
        s.character(key.input(), false);
    }

    void updateStrokeWildcardKey() {
        if (s.strokeWildcardKey == null) return;
        s.strokeWildcardKey.setEnabled(
            StrokeKeyboardLayout.sends(StrokeKeyboardLayout.WILDCARD, s.hasEngineComposition()));
    }

    /** Show the digit and literal letters printed on a nine-key key, like Apple's hold popup. */
    void showNineKeyHoldOptions(Button anchor, NineKeyLayout.Key key) {
        dismissNineKeyHoldOptions();
        if (s.keyboardRoot == null) return;
        s.imeKeyFeedback.playFeedback(anchor);
        LinearLayout options = new LinearLayout(s);
        options.setOrientation(LinearLayout.HORIZONTAL);
        int padding = s.pixels(5);
        options.setPadding(padding, padding, padding, padding);
        GradientDrawable surface = new GradientDrawable();
        surface.setColor(Color.parseColor(s.skin.background()));
        surface.setCornerRadius(s.pixels(10));
        surface.setStroke(Math.max(1, s.pixels(1)), Color.parseColor(s.skin.accent()));
        options.setBackground(surface);

        String letters = key.label().toLowerCase(java.util.Locale.ROOT);
        String[] choices = new String[letters.length() + 1];
        choices[0] = String.valueOf(key.input());
        for (int index = 0; index < letters.length(); index++)
            choices[index + 1] = String.valueOf(letters.charAt(index));
        for (String choice : choices) {
            Button option = s.keyboardKey(choice, "输入 " + choice,
                () -> commitNineKeyHoldOption(choice));
            option.setContentDescription("输入 " + choice);
            option.setPadding(0, 0, 0, 0);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                s.pixels(36), s.pixels(38));
            if (options.getChildCount() > 0) params.setMarginStart(s.pixels(2));
            options.addView(option, params);
        }

        options.measure(View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        final PopupWindow[] holder = new PopupWindow[1];
        // 弹窗不能取焦点：输入法里一个可取焦点的窗口会把焦点从宿主的输入框抢走，编辑器一失焦系统就收起整个键盘。点外面关闭由 `setOutsideTouchable` 负责。
        PopupWindow popup = new PopupWindow(options, options.getMeasuredWidth(),
            options.getMeasuredHeight(), false);
        holder[0] = popup;
        popup.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
        popup.setOutsideTouchable(true);
        popup.setClippingEnabled(true);
        popup.setElevation(s.pixels(4));
        popup.setOnDismissListener(() -> {
            if (s.nineKeyHoldPopup == holder[0]) s.nineKeyHoldPopup = null;
        });
        s.nineKeyHoldPopup = popup;
        int xOffset = (anchor.getWidth() - options.getMeasuredWidth()) / 2;
        int yOffset = -anchor.getHeight() - options.getMeasuredHeight() - s.pixels(6);
        popup.showAsDropDown(anchor, xOffset, yOffset);
    }

    void commitNineKeyHoldOption(String text) {
        dismissNineKeyHoldOptions();
        if (s.connection == null) return;
        s.command(2);
        s.commitText(s.fullWidthOutput(text));
    }

    void dismissNineKeyHoldOptions() {
        if (s.nineKeyHoldPopup != null) {
            s.nineKeyHoldPopup.dismiss();
            s.nineKeyHoldPopup = null;
        }
    }

    static String japaneseKeyLabel(JapaneseNineKeyLayout.Key key) {
        return key.kana().get(0) + "\n" + key.kana().subList(1, 5).stream()
            .filter(label -> !label.isEmpty()).collect(java.util.stream.Collectors.joining(" "));
    }

    void inputJapaneseStroke(String input) {
        if (s.session == 0 || input.isEmpty()) return;
        for (int index = 0; index < input.length(); index++) s.character(input.charAt(index));
    }

    void selectJapaneseKey(JapaneseNineKeyLayout.Key key, int direction) {
        if (direction < 0 || direction >= key.kana().size()) return;
        if (key.kana().get(direction).isEmpty()) return;
        String stroke = key.strokes().get(direction);
        if (stroke.isEmpty()) commitNineKeyLiteral(key.kana().get(direction));
        else inputJapaneseStroke(stroke);
    }

    void showJapaneseFlickPreview(Button button, JapaneseNineKeyLayout.Key key, int direction) {
        if (s.japaneseFlickPreview != null && s.keyboardRoot != null)
            s.japaneseFlickPreview.show(button, key, direction, s.keyboardRoot);
    }

    void hideJapaneseFlickPreview() {
        if (s.japaneseFlickPreview != null) s.japaneseFlickPreview.hide();
    }

    void bindJapaneseFlick(Button button, JapaneseNineKeyLayout.Key key) {
        final float[] origin = new float[2];
        final int[] direction = new int[1];
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    direction[0] = 0;
                    button.setPressed(true);
                    showJapaneseFlickPreview(button, key, 0);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    direction[0] = JapaneseNineKeyLayout.direction(
                        event.getX() - origin[0], event.getY() - origin[1], s.pixels(12));
                    showJapaneseFlickPreview(button, key, direction[0]);
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    button.setPressed(false);
                    hideJapaneseFlickPreview();
                    if (direction[0] == 0) button.performClick();
                    else {
                        s.imeKeyFeedback.playFeedback(button);
                        s.countKey(button);
                        selectJapaneseKey(key, direction[0]);
                    }
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    button.setPressed(false);
                    hideJapaneseFlickPreview();
                    return true;
                }
                default -> {
                    return true;
                }
            }
        });
    }

    Button japaneseKey(JapaneseNineKeyLayout.Key key) {
        String description = key.kana().stream().filter(label -> !label.isEmpty())
            .collect(java.util.stream.Collectors.joining("、"));
        Button button = s.keyboardKey(japaneseKeyLabel(key), description,
            () -> selectJapaneseKey(key, 0));
        button.setContentDescription("轻点输入" + key.kana().get(0)
            + "；左、上、右、下滑动选择其他假名");
        bindJapaneseFlick(button, key);
        return button;
    }

    void showJapaneseBracketOptions(Button anchor) {
        PopupMenu popup = new PopupMenu(s, anchor);
        for (String bracket : JapaneseNineKeyLayout.digitBrackets()) {
            popup.getMenu().add(bracket).setOnMenuItemClickListener(ignored -> {
                s.imeKeyFeedback.playFeedback(anchor);
                commitNineKeyLiteral(bracket);
                return true;
            });
        }
        popup.show();
    }

    Button japaneseVariantsKey() {
        boolean symbols = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
        Button variants = s.keyboardKey(symbols ? "（）" : "小゛゜",
            symbols ? "括号；长按选择其他括号" : "小假名、浊音和半浊音", () -> {});
        s.japaneseVariantsButton = variants;
        variants.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(variants);
            if (s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS) showJapaneseBracketOptions(variants);
            else s.command(MSIMEInputService.CYCLE_KANA_VARIANT_COMMAND);
        });
        return variants;
    }

    void addJapaneseSideKey(LinearLayout column, Button button, float weight) {
        column.addView(button, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, weight));
    }

    void rebuildJapaneseNineKeyRows() {
        LinearLayout container = new LinearLayout(s);
        container.setOrientation(LinearLayout.HORIZONTAL);
        s.imeStyler.adjustFixedHeight(container, KeyboardGeometry.NINE_KEY_HEIGHT_DP);
        s.keyRows.addView(container, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(180)));

        LinearLayout modeColumn = new LinearLayout(s);
        modeColumn.setOrientation(LinearLayout.VERTICAL);
        s.japaneseSymbolsKey = s.keyId(s.keyboardKey("123", "切换到数字和符号", () -> {
            s.keyboardLayer = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                ? KeyboardLayout.Layer.LETTERS : KeyboardLayout.Layer.SYMBOLS;
            s.imeLetterRows.rebuildKeyRows();
            s.render();
        }), "SoftLayer");
        addJapaneseSideKey(modeColumn, s.japaneseSymbolsKey, 1);
        addJapaneseSideKey(modeColumn, s.keyId(s.keyboardKey("☺", "打开表情浏览", s.imePanels::showEmojiPicker),
            "SoftEmoji"), 1);
        Button language = s.keyId(s.keyboardKey("英", "切换到英文输入", s::toggleInputLanguage),
            "SoftLanguage");
        addJapaneseSideKey(modeColumn, language,
            s.shouldOfferSwitchingToNextInputMethod() ? 1 : 2);
        if (s.shouldOfferSwitchingToNextInputMethod()) {
            addJapaneseSideKey(modeColumn, s.keyId(s.keyboardKey("切换", "切换到下一个输入法",
                s::switchToNextInputMethodAfterCommit), "SoftGlobe"), 1);
        }
        container.addView(modeColumn, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.17f));

        LinearLayout grid = new LinearLayout(s);
        grid.setOrientation(LinearLayout.VERTICAL);
        java.util.List<JapaneseNineKeyLayout.Key> keys = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
            ? JapaneseNineKeyLayout.digitKeys() : JapaneseNineKeyLayout.keys();
        for (int rowIndex = 0; rowIndex < 4; rowIndex++) {
            LinearLayout row = new LinearLayout(s);
            if (rowIndex < 3) {
                for (int column = 0; column < 3; column++) {
                    int index = rowIndex * 3 + column;
                    addNineKey(row, s.keyId(japaneseKey(keys.get(index)),
                        KeyPressIds.forJapaneseKeyIndex(index)));
                }
            } else {
                addNineKey(row, japaneseVariantsKey());
                addNineKey(row, s.keyId(japaneseKey(keys.get(9)), KeyPressIds.forJapaneseKeyIndex(9)));
                addNineKey(row, s.keyId(japaneseKey(keys.get(10)), KeyPressIds.forJapaneseKeyIndex(10)));
            }
            grid.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        container.addView(grid, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.64f));

        LinearLayout side = new LinearLayout(s);
        side.setOrientation(LinearLayout.VERTICAL);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.keyboardKey("⌫", "删除", deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        addJapaneseSideKey(side, delete, 1);
        s.japaneseSpaceKey = s.keyId(s.keyboardKey("空白", "空白；左右滑动移动光标", s::space), "Space");
        s.imeBottomRow.bindSpaceCursor(s.japaneseSpaceKey);
        addJapaneseSideKey(side, s.japaneseSpaceKey, 1);
        s.japaneseReturnKey = s.keyId(s.keyboardKey("改行", "改行", s::enter), "Enter");
        addJapaneseSideKey(side, s.japaneseReturnKey, 2);
        container.addView(side, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.19f));
        s.updateReturnKey();
    }

    void commitNineKeyLiteral(String text) {
        if (s.connection == null) return;
        s.command(2);
        s.commitText(s.fullWidthOutput(text));
    }

    void commitNineKeyPeriod() {
        if (s.dedicatedEnglish || !s.punctuation('.')) commitNineKeyLiteral(".");
    }

    void chooseNineKeySpelling(long generation, int index) {
        if (s.session == 0) return;
        try { s.apply(NativeClient.chooseNineKeySpelling(s.session, generation, index)); }
        catch (JSONException | LinkageError error) { s.fail(); }
    }

    void renderNineKeySpellings() {
        if (s.nineKeySpellings == null || s.nineKeySpellingScroll == null) return;
        JSONArray spellings = s.view == null ? null : s.view.optJSONArray("nine_key_spellings");
        boolean visible = s.displayedTouchLayout(s.view) == MSIMEInputService.QUANPIN_NINE_KEY_LAYOUT
            && spellings != null && spellings.length() > 0;
        s.nineKeySpellingScroll.setVisibility(visible ? View.VISIBLE : View.GONE);
        if (!visible) {
            s.nineKeySpellingIndices = java.util.List.of();
            s.nineKeySpellingGeneration = -1;
            for (Button key : s.nineKeySpellingButtons) key.setVisibility(View.GONE);
            return;
        }
        s.nineKeySpellingGeneration = CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1);
        java.util.List<String> values = new java.util.ArrayList<>();
        java.util.List<Integer> indices = new java.util.ArrayList<>();
        for (int index = 0; index < spellings.length(); index++) {
            String spelling = spellings.optString(index, "");
            if (!spelling.isEmpty()) {
                values.add(spelling);
                indices.add(index);
            }
        }
        s.nineKeySpellingIndices = java.util.List.copyOf(indices);
        while (s.nineKeySpellingButtons.size() < values.size()) {
            int slot = s.nineKeySpellingButtons.size();
            Button key = s.button(s.nineKeySpellings, "", () -> {
                if (slot < s.nineKeySpellingIndices.size()) {
                    chooseNineKeySpelling(s.nineKeySpellingGeneration,
                        s.nineKeySpellingIndices.get(slot));
                }
            });
            key.setContentDescription("选择拼音");
            LinearLayout.LayoutParams params = (LinearLayout.LayoutParams) key.getLayoutParams();
            params.width = LinearLayout.LayoutParams.WRAP_CONTENT;
            params.weight = 0;
            key.setLayoutParams(params);
            s.nineKeySpellingButtons.add(key);
        }
        for (int slot = 0; slot < s.nineKeySpellingButtons.size(); slot++) {
            Button key = s.nineKeySpellingButtons.get(slot);
            boolean slotVisible = slot < values.size();
            key.setVisibility(slotVisible ? View.VISIBLE : View.GONE);
            if (slotVisible) {
                String spelling = values.get(slot);
                key.setText(spelling);
                key.setContentDescription("选择拼音 " + spelling);
            }
        }
    }
}
