package app.msime.android;

import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.PopupWindow;
import android.widget.TextView;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 26 键以外的键区：手写、九键、笔画、注音 9 键、日语九键（含 flick 与长按选项）、侧栏与九键拼音 / 注音读音选择；从 MSIMEInputService 原样搬出。 */
final class ImeLayoutRows {
    private final MSIMEInputService s;

    ImeLayoutRows(MSIMEInputService s) {
        this.s = s;
    }

    /** 键盘服务自己的停笔防抖（MSIMEInputService.HANDWRITING_DEBOUNCE_MILLIS）；识别等待时间比它长时由这里补足差值。 */
    static final long SERVICE_HANDWRITING_DEBOUNCE_MILLIS = 550;
    /** 叠写自动上屏后，「已上屏」提示停留多久再回到书写提示。 */
    static final long HANDWRITING_COMMITTED_NOTICE_MILLIS = 1500;
    private HandwritingPreferences handwritingPreferences = HandwritingPreferences.defaults();
    private Runnable pendingInk;
    private Runnable handwritingNoticeReset;
    private long pinyinGeneration;
    private final java.util.concurrent.ExecutorService pinyinWorker =
        java.util.concurrent.Executors.newSingleThreadExecutor(runnable -> {
            Thread thread = new Thread(runnable, "msime-handwriting-pinyin");
            thread.setDaemon(true);
            return thread;
        });

    /** 服务销毁时调用：作废还没回来的手写拼音并停掉它的线程。 */
    void shutdown() {
        pinyinGeneration++;
        pinyinWorker.shutdownNow();
    }

    /** 读本地设置里的手写细项；没写过的取默认值。 */
    HandwritingPreferences readHandwritingPreferences() {
        AndroidLocalSettings.Snapshot settings = s.localSettings;
        return HandwritingPreferences.of(settings.choice(AndroidLocalSettings.HANDWRITING_MODE),
            settings.integer(AndroidLocalSettings.HANDWRITING_DELAY_MS),
            settings.bool(AndroidLocalSettings.HANDWRITING_SHOW_PINYIN),
            settings.choice(AndroidLocalSettings.HANDWRITING_STROKE_COLOR),
            settings.integer(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH));
    }

    /**
     * 手写键区：左列 ，。？！，中间书写区（「在此手写，停笔后选字」），右列 ⌫ 与「重写」。底行（123 / 写 / 空格 / ↵）由底行构建负责。
     *
     * <p>按 `touch_handwriting` 生效：单字一字一识别；叠写停笔后识别并自动上屏首选、清空画布；行写整行一次识别并把光标前的上文交给识别器。识别等待时间比服务的防抖长时补足差值；识别后可在书写区下方显示首选的拼音；笔迹颜色与粗细跟偏好。
     */
    void rebuildHandwritingRows() {
        cancelPendingInk();
        handwritingPreferences = readHandwritingPreferences();
        s.handwritingStatus = ViewPolicy.centeredText(s, "在此手写，停笔后选字", 13);
        KeyboardGeometry.setKeyTextSize(s.handwritingStatus, 13);
        s.handwritingStatus.setContentDescription("手写状态");
        ViewPolicy.setNonInteractive(s.handwritingStatus);

        LinearLayout row = KeyboardGeometry.row(s);

        LinearLayout punctuation = KeyboardGeometry.column(s);
        for (String symbol : NineKeyLayout.punctuation()) {
            Button key = s.keyId(s.keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            // 全角「，」「。」的墨迹只占字身左下角，直接当键面文字会缩成贴底的小点。
            if ("，".equals(symbol) || "。".equals(symbol)) CenteredGlyphSpan.apply(key, symbol, 1.3f);
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
            addNineKey(punctuation, key);
        }
        row.addView(punctuation, KeyboardGeometry.weightedMatchParentParams(0.7f));

        FrameLayout area = new FrameLayout(s);
        s.handwritingCanvas = new HandwritingCanvas(s);
        s.handwritingCanvas.applySkin(s.handwritingSkin);
        s.handwritingCanvas.setInk(handwritingPreferences.inkColor(),
            s.pixels(handwritingPreferences.strokeWidth()));
        area.addView(s.handwritingCanvas, KeyboardGeometry.frameMatchParentParams());

        FrameLayout cardFrame = new FrameLayout(s);
        ViewPolicy.setNonInteractive(cardFrame);
        FrameLayout.LayoutParams cardParams = KeyboardGeometry.frameMatchParentParams();
        int inset = s.pixels(3);
        cardParams.setMargins(inset, inset, inset, inset);
        area.addView(cardFrame, cardParams);
        cardFrame.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> s.handwritingCanvas.setCardRect(
                    left, top, right, bottom));
        cardFrame.addView(s.handwritingStatus, KeyboardGeometry.frameMatchWidthWrapParams(
            Gravity.CENTER));

        s.handwritingDownload = new Button(s);
        ViewPolicy.setAllCapsFalse(s.handwritingDownload);
        KeyboardGeometry.setKeyTextSize(s.handwritingDownload, KeyboardGeometry.DEFAULT_KEY_TEXT_SP);
        ViewPolicy.setSingleLine(s.handwritingDownload);
        bindFeedbackAction(s.handwritingDownload, s::downloadHandwritingModel);
        FrameLayout.LayoutParams downloadParams = KeyboardGeometry.frameMatchWidthHeightPx(s.pixels(48));
        downloadParams.gravity = Gravity.CENTER;
        downloadParams.leftMargin = s.pixels(16);
        downloadParams.rightMargin = s.pixels(16);
        cardFrame.addView(s.handwritingDownload, downloadParams);
        row.addView(area, KeyboardGeometry.weightedMatchParentParams(3f));

        LinearLayout tools = KeyboardGeometry.column(s);
        Button delete = s.keyId(s.backspaceKey(s::deleteFromHandwriting), "Backspace");
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(tools, delete);
        Button rewrite = s.keyboardKey("重写", "清空手写", () -> {
            cancelPendingInk();
            s.clearHandwriting();
        });
        if (rewrite instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(tools, rewrite);
        row.addView(tools, KeyboardGeometry.weightedMatchParentParams(0.8f));
        // 手写区和其他布局的三行键一样高，切换布局时键盘总高度不跳。
        s.imeStyler.adjustThreeRowBlockHeight(row);
        s.keyRows.addView(row);

        s.handwritingRecognizer = new HandwritingResultTap(HandwritingRecognizerFactory.create(s),
            (revision, candidates) -> s.main.post(() -> onHandwritingRecognized(revision, candidates)));
        s.handwritingCanvas.setListener(new HandwritingCanvas.Listener() {
            @Override public void onStrokeBegan() {
                cancelPendingInk();
                cancelHandwritingNotice();
                s.invalidateHandwritingRecognition();
                s.showHandwritingStatus("书写中…");
            }

            @Override public void onInkChanged(long revision,
                    java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
                handwritingInkChanged(revision, strokes);
            }
        });
        s.refreshHandwritingAvailability();
    }

    /** 停笔：行写时先把上文交给识别器；识别等待时间比服务的防抖长时先等差值，再交给服务的识别流程。 */
    private void handwritingInkChanged(long revision,
                                       java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
        cancelPendingInk();
        if (s.handwritingRecognizer != null) {
            s.handwritingRecognizer.setPreContext(
                handwritingPreferences.mode() == HandwritingPreferences.Mode.LINE ? textBeforeCursor() : "");
        }
        long extra = handwritingPreferences.extraDelay(SERVICE_HANDWRITING_DEBOUNCE_MILLIS);
        if (extra == 0 || strokes.isEmpty()) {
            s.handwritingInkChanged(revision, strokes);
            return;
        }
        s.invalidateHandwritingRecognition();
        s.showHandwritingStatus("停笔后识别…");
        Runnable task = new Runnable() {
            @Override public void run() {
                if (pendingInk != this) return;
                pendingInk = null;
                if (s.handwritingCanvas == null || s.handwritingCanvas.revision() != revision) return;
                s.handwritingInkChanged(revision, strokes);
            }
        };
        pendingInk = task;
        s.main.postDelayed(task, extra);
    }

    private void cancelPendingInk() {
        if (pendingInk != null) s.main.removeCallbacks(pendingInk);
        pendingInk = null;
    }

    private void cancelHandwritingNotice() {
        if (handwritingNoticeReset != null) s.main.removeCallbacks(handwritingNoticeReset);
        handwritingNoticeReset = null;
    }

    /** 光标前的上文，供行写识别；读不到时为空串。 */
    private String textBeforeCursor() {
        if (s.connection == null) return "";
        CharSequence before = s.connection.getTextBeforeCursor(HandwritingRecognizer.MAX_PRE_CONTEXT, 0);
        return HandwritingRecognizer.clipPreContext(before == null ? "" : before.toString());
    }

    /** 识别结果到了（主线程，排在服务显示候选之后）：叠写自动上屏首选并清空画布；需要时显示首选的拼音。 */
    private void onHandwritingRecognized(long revision, java.util.List<String> values) {
        if (!s.handwritingActive() || s.handwritingCanvas == null
                || s.handwritingCanvas.revision() != revision) return;
        java.util.List<String> candidates = HandwritingRecognizer.sanitizeCandidates(values);
        if (candidates.isEmpty()) return;
        String first = candidates.get(0);
        if (handwritingPreferences.mode() == HandwritingPreferences.Mode.OVERLAP) {
            if (s.connection == null) return;
            long targetSession = s.session;
            s.command(2);
            if (targetSession != s.session || !s.handwritingActive()) return;
            if (!s.commitText(s.chineseOutput(first, s.view), TypingSource.HANDWRITING)) return;
            s.clearHandwriting();
            showHandwritingNotice("已上屏：" + first, first, s.handwritingCanvas.revision());
            return;
        }
        if (handwritingPreferences.showPinyin()) showHandwritingNotice(null, first, revision);
    }

    /**
     * 在书写区显示一条识别提示：叠写上屏后显示「已上屏：字」，过一会儿回到书写提示；打开了「识别后显示拼音」时在后面接上拼音（内置词库查出的规范读音）。
     *
     * @param prefix 先显示的文字；null 表示只显示「字 拼音」
     */
    private void showHandwritingNotice(String prefix, String text, long revision) {
        cancelHandwritingNotice();
        if (prefix != null) s.showHandwritingStatus(prefix);
        boolean committed = prefix != null;
        if (committed) {
            Runnable reset = new Runnable() {
                @Override public void run() {
                    if (handwritingNoticeReset != this) return;
                    handwritingNoticeReset = null;
                    if (s.handwritingCanvas != null && !s.handwritingCanvas.hasInk())
                        s.showHandwritingStatus("在此手写，停笔后选字");
                }
            };
            handwritingNoticeReset = reset;
            s.main.postDelayed(reset, HANDWRITING_COMMITTED_NOTICE_MILLIS);
        }
        if (!handwritingPreferences.showPinyin()) return;
        String resources = s.emojiResources;
        if (resources == null || resources.isEmpty()) return;
        long generation = ++pinyinGeneration;
        try {
            pinyinWorker.execute(() -> {
                String pinyin = pinyinOf(text, resources);
                if (pinyin.isEmpty()) return;
                s.main.post(() -> {
                    if (generation != pinyinGeneration || s.handwritingCanvas == null
                            || s.handwritingCanvas.revision() != revision) return;
                    if (committed && handwritingNoticeReset == null) return;
                    s.showHandwritingStatus((committed ? prefix : text) + "  " + pinyin);
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException ignored) {
            // 键盘正在退出，不再显示拼音。
        }
    }

    /** 内置词库里这个词的规范读音（多个音节以空格分开）；查不到时为空串。在工作线程调用。 */
    static String pinyinOf(String text, String resources) {
        try {
            JSONObject root = new JSONObject(NativeClient.dictionaryHansEntries(text, resources));
            JSONObject value = Boolean.TRUE.equals(root.opt("ok"))
                ? root.optJSONObject("value") : null;
            JSONArray entries = value == null ? null : value.optJSONArray("entries");
            JSONObject entry = entries == null || entries.length() == 0 ? null : entries.optJSONObject(0);
            return entry == null ? ""
                : TextPolicy.trimmed(entry.optString("key", "").replace('\'', ' '));
        } catch (JSONException | RuntimeException | LinkageError error) {
            return "";
        }
    }

    void addNineKey(LinearLayout parent, Button key) {
        boolean horizontal = parent.getOrientation() == LinearLayout.HORIZONTAL;
        parent.addView(key, KeyboardGeometry.weightedCrossAxisFillParams(horizontal, 1));
    }

    /** The rail behind the capless punctuation column; it is not a Button, so the skin pass misses it. */
    void applySidebarRail() {
        if (s.nineKeySidebar == null) return;
        GradientDrawable rail = DrawablePolicy.rounded(
            Color.parseColor(s.skin.sidebarBackground()), s.pixels(s.skin.cornerRadius()));
        s.nineKeySidebar.setBackground(rail);
    }

    void rebuildNineKeyRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = KeyboardGeometry.row(s);
        s.imeStyler.adjustThreeRowBlockHeight(container);
        s.keyRows.addView(container, KeyboardGeometry.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3)));

        LinearLayout punctuation = KeyboardGeometry.column(s);
        // 设计里左列是 ，。？ 三个，！ 放在右列最下面，和 3×3 网格逐行对齐。
        java.util.List<String> symbols = NineKeyLayout.punctuation();
        for (String symbol : symbols.subList(0, symbols.size() - 1)) {
            Button key = s.keyId(s.keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            // 全角「，」「。」的墨迹只占字身左下角，直接当键面文字会缩成贴底的小点。
            if ("，".equals(symbol) || "。".equals(symbol)) CenteredGlyphSpan.apply(key, symbol, 1.3f);
            // The punctuation keys share one rail rather than wearing caps of their own.
            if (key instanceof KeyboardPressButton press)
                press.setKeyboardRole(KeyboardKeyRole.PLAIN);
            punctuation.addView(key, KeyboardGeometry.weightedWidthParams(1));
        }
        FrameLayout sidebar = new FrameLayout(s);
        s.nineKeySidebar = sidebar;
        applySidebarRail();
        sidebar.addView(punctuation, KeyboardGeometry.frameMatchParentParams());
        if (s.nineKeySpellingScroll != null) {
            // 拼音选择条只在创建键盘视图时建一次，每次重建九键都会换一个新的侧栏；偏好变化触发第二次重建时它还挂在上一个侧栏上，不先摘下来，addView 会抛 IllegalStateException 让键盘进程崩溃。
            if (s.nineKeySpellingScroll.getParent() instanceof android.view.ViewGroup previous)
                previous.removeView(s.nineKeySpellingScroll);
            sidebar.addView(s.nineKeySpellingScroll, KeyboardGeometry.frameMatchParentParams());
            placeSpellingRow(false);
        }
        container.addView(sidebar, KeyboardGeometry.weightedMatchParentParams(0.7f));

        LinearLayout grid = KeyboardGeometry.column(s);
        boolean digits = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
        for (java.util.List<NineKeyLayout.Key> keys : NineKeyLayout.rows()) {
            LinearLayout row = KeyboardGeometry.row(s);
            for (NineKeyLayout.Key key : keys) {
                String description = NineKeyLayout.description(key, digits);
                // On the digit layer the grid is a numeric keypad, so a tap commits the number
                // instead of feeding it to the pinyin session.
                NineKeyDigitButton keyButton = s.nineKeyGridKey(
                    NineKeyLayout.face(key, digits), description,
                    digits ? () -> commitNineKeyLiteral(NineKeyLayout.digitInput(key))
                        : NineKeyLayout.opensSymbols(key, false) ? s.imePanels::showSymbolPanel
                        : () -> s.character(key.input()));
                s.keyId(keyButton, KeyPressIds.forNineKeyDigit(key.digit()));
                // 字母键面上印着它送进引擎的数字；数字键面本身就是那个数字，不必再印一次。
                // 1 键在拼音键面上是「@#」，打开符号面板而不是送 1，所以它没有可印的数字。
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
            grid.addView(row, KeyboardGeometry.weightedWidthParams(1));
        }
        container.addView(grid, KeyboardGeometry.weightedMatchParentParams(3));

        LinearLayout actions = KeyboardGeometry.column(s);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, delete);
        // 拆分：组字时把 ' 送给引擎，在已打的数字末尾定一个音节分界，只定在哪里断、不定是哪个拼音：94 拆分 26 仍可以是 xi'an（西安）或 yi'an，但不再是 xian（先）。读音栏显示成 94'26；⌫ 先删分界再删数字。原来锁定拼音列第一项的做法会把 94 定成首选的 yi，打不出西安。没在组字时引擎不处理，什么也不发生。原来的「重输」改为长按 ⌫：组字时长按删除键丢掉整串拼音（bindBackspaceRepeat）。
        Button split = s.keyboardKey("拆分", "拆分音节", () -> {
            if (s.view != null && !s.view.optString("editing_text", "").isEmpty()) s.character('\'', false);
        });
        if (split instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, split);
        String last = symbols.get(symbols.size() - 1);
        Button exclamation = s.keyId(s.keyboardKey(last, "符号 " + last,
            () -> commitNineKeyLiteral(last)), "SoftPunctuation");
        if (exclamation instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, exclamation);
        container.addView(actions, KeyboardGeometry.weightedMatchParentParams(0.8f));
    }

    /**
     * 笔画键盘：九键外框（标点侧栏、⌫ 列、固定高度）中间换成 {@link StrokeKeyboardLayout} 的 2×3 笔画网格。
     *
     * <p>笔画键直接走 character()，不走 type()：type() 会套用 Shift 大小写，也会把 ASCII 标点交给标点路径。九键的拼音选择条只属于拼音九键，这里不挂（挂上去要先从旧侧栏摘下，否则 addView 会抛异常）。
     */
    void rebuildStrokeRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = KeyboardGeometry.row(s);
        s.imeStyler.adjustThreeRowBlockHeight(container);
        s.keyRows.addView(container, KeyboardGeometry.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3)));

        LinearLayout punctuation = KeyboardGeometry.column(s);
        // 外框和拼音九键一样三行对齐：左列 ，。？，！ 在右列最下；中间两行笔画下面再一行 @#、0、句点。原来左列四个、中间两行、右列三个，三列互不对齐，看起来像少了一行。
        java.util.List<String> symbols = NineKeyLayout.punctuation();
        for (String symbol : symbols.subList(0, symbols.size() - 1)) {
            Button key = s.keyId(s.keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            // 全角「，」「。」的墨迹只占字身左下角，直接当键面文字会缩成贴底的小点。
            if ("，".equals(symbol) || "。".equals(symbol)) CenteredGlyphSpan.apply(key, symbol, 1.3f);
            if (key instanceof KeyboardPressButton press)
                press.setKeyboardRole(KeyboardKeyRole.PLAIN);
            punctuation.addView(key, KeyboardGeometry.weightedWidthParams(1));
        }
        FrameLayout sidebar = new FrameLayout(s);
        s.nineKeySidebar = sidebar;
        applySidebarRail();
        sidebar.addView(punctuation, KeyboardGeometry.frameMatchParentParams());
        container.addView(sidebar, KeyboardGeometry.weightedMatchParentParams(0.7f));

        LinearLayout grid = KeyboardGeometry.column(s);
        for (java.util.List<StrokeKeyboardLayout.Key> keys : StrokeKeyboardLayout.rows()) {
            LinearLayout row = KeyboardGeometry.row(s);
            for (StrokeKeyboardLayout.Key key : keys) {
                Button keyButton = s.keyboardKey(StrokeKeyboardLayout.face(key),
                    StrokeKeyboardLayout.accessibilityLabel(key), () -> strokeKey(key));
                keyButton.setContentDescription(StrokeKeyboardLayout.accessibilityLabel(key));
                // 笔画本身的墨迹很细（一、丨、丶），按普通键面字号排只剩一道短线，字形放大，下面的名称仍是一半大小。
                KeyboardGeometry.setKeyTextSize(keyButton, 26);
                twoLineFace(keyButton, StrokeKeyboardLayout.face(key));
                if (keyButton instanceof KeyboardPressButton press)
                    press.setKeyboardRole(KeyboardKeyRole.KEY);
                s.keyId(keyButton, KeyPressIds.forCharacter(key.input()));
                if (key.input() == StrokeKeyboardLayout.WILDCARD) s.strokeWildcardKey = keyButton;
                addNineKey(row, keyButton);
            }
            grid.addView(row, KeyboardGeometry.weightedWidthParams(1));
        }
        LinearLayout lastRow = KeyboardGeometry.row(s);
        Button symbolsKey = s.keyId(s.keyboardKey("@#", "符号", s.imePanels::showSymbolPanel), KeyPressIds.forNineKeyDigit(1));
        Button zero = s.keyId(s.keyboardKey("0", "数字 0", () -> commitNineKeyLiteral("0")), "Nine0");
        Button period = s.keyId(s.keyboardKey(".", "句点", this::commitNineKeyPeriod), "Period");
        for (Button key : java.util.List.of(symbolsKey, zero, period)) {
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
            addNineKey(lastRow, key);
        }
        grid.addView(lastRow, KeyboardGeometry.weightedWidthParams(1));
        container.addView(grid, KeyboardGeometry.weightedMatchParentParams(3));

        LinearLayout actions = KeyboardGeometry.column(s);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.connection.deleteSurroundingTextInCodePoints(1, 0);
        };
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, delete);
        // 右列和拼音九键一样三等分（⌫、拆分、！）、与三行网格逐行对齐；原来 ⌫ 独占两行，看起来像别的键盘拼进来的。笔画不需要拆分，中间这格是「重输」：丢掉已打的整串笔画。
        Button rewrite = s.keyboardKey("重输", "重新输入笔画", () -> {
            if (s.hasEngineComposition()) s.discardComposition();
        });
        if (rewrite instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, rewrite);
        String last = symbols.get(symbols.size() - 1);
        Button exclamation = s.keyId(s.keyboardKey(last, "符号 " + last,
            () -> commitNineKeyLiteral(last)), "SoftPunctuation");
        if (exclamation instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, exclamation);
        container.addView(actions, KeyboardGeometry.weightedMatchParentParams(0.8f));
        updateStrokeWildcardKey();
    }

    /**
     * 把底栏的一个常驻键（123、中/英、空格、换行）挂进本布局的一列：先从原来的父视图摘下，按底栏同样的规矩给角色、样式和字号。没有底栏的布局（注音九键）切走时，{@link ImeBottomRow#updateActionRow} 会把它们收回底栏。
     */
    private void adoptBarKey(LinearLayout parent, Button key, KeyboardKeyRole role) {
        if (key == null) return;
        if (key.getParent() instanceof android.view.ViewGroup previous) previous.removeView(key);
        if (key instanceof KeyboardPressButton press) press.setKeyboardRole(role);
        s.imeStyler.styleButton(key, role, s.skin);
        if (role == KeyboardKeyRole.ACCENT) KeyboardGeometry.setKeyTextSize(key, 15);
        ViewPolicy.show(key);
        addNineKey(parent, key);
    }

    /**
     * 注音 9 键：与拼音九键、笔画同一个三行高的外框，左列五个声调键，中间 1-9 三行音键加最后一行 @#、0、，。（四行挤进三行高，和大千的四行一样），右列 ⌫（两格高）、？、！。底行照常是 {@link KeyboardActionRow#designEntries}，逗号句号已在网格里，底行不再放。
     *
     * <p>音键和声调键直接走 character()，不走 type()：type() 会套用 Shift 大小写。音键发送数字，声调键发送 z x c v b，Engine 的注音九键编辑器把它们读作 ˉ ˊ ˇ ˋ ˙；底行空格同样是一声。读音选择条不放进左列（声调键在组字时要一直可按），而是叠在候选行上：注音的候选只在打开列表后才出现，列表关着时那一行是空的，render 在选择条显示时把候选滚动区让成不可见。
     */
    void rebuildZhuyinNineKeyRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = KeyboardGeometry.row(s);
        // 和日语九键一样不要底栏，四行都在这一块里，总高度是三行九键加一条底栏，与其他九键键盘一样高：四行挤进三行高时键太扁。底栏的 123、中/英、空格、换行挪进两侧的列。
        s.imeStyler.adjustBottomRowBlockHeight(container);
        s.keyRows.addView(container, KeyboardGeometry.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3 + KeyboardGeometry.STANDARD_ROW_HEIGHT_DP)));

        LinearLayout tones = KeyboardGeometry.column(s);
        for (ZhuyinNineKeyLayout.Tone tone : ZhuyinNineKeyLayout.tones()) {
            String label = ZhuyinNineKeyLayout.accessibilityLabel(tone);
            Button key = s.keyId(s.keyboardKey(tone.face(), label, () -> {
                if (s.connection != null && ZhuyinInputPolicy.toneKeySends(
                        s.view == null ? "" : s.view.optString("editing_text", "")))
                    s.character(tone.input(), false);
            }), tone.keyId());
            key.setContentDescription(label);
            // 声调符号本身只是一道短笔画，按默认字号画出来几乎看不见。
            KeyboardGeometry.setKeyTextSize(key, 20);
            CenteredGlyphSpan.apply(key, tone.face(), 1f);
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
            addNineKey(tones, key);
        }
        container.addView(tones, KeyboardGeometry.weightedMatchParentParams(0.7f));

        LinearLayout grid = KeyboardGeometry.column(s);
        for (java.util.List<ZhuyinNineKeyLayout.Key> keys : ZhuyinNineKeyLayout.rows()) {
            LinearLayout row = KeyboardGeometry.row(s);
            for (ZhuyinNineKeyLayout.Key key : keys) addNineKey(row, zhuyinNineKeySoundKey(key));
            grid.addView(row, KeyboardGeometry.weightedWidthParams(1));
        }
        // 最后一行：123（数字与符号层，？！ 和其他符号都在那里）、0（仍在 8 的正下方）、逗号和句号各占半格。
        LinearLayout lastRow = KeyboardGeometry.row(s);
        adoptBarKey(lastRow, s.layerButton, KeyboardKeyRole.ACCENT);
        addNineKey(lastRow, zhuyinNineKeySoundKey(ZhuyinNineKeyLayout.zero()));
        LinearLayout marks = KeyboardGeometry.row(s);
        boolean chinese = s.sendsChinesePunctuation();
        String commaFace = ChineseSymbolFaces.face(",", chinese);
        Button comma = s.keyId(s.keyboardKey(commaFace, "逗号", () -> {
            if (s.dedicatedEnglish || !s.punctuation(',')) commitNineKeyLiteral(",");
        }), KeyPressIds.forCharacter(','));
        String periodFace = ChineseSymbolFaces.face(".", chinese);
        Button period = s.keyId(s.keyboardKey(periodFace, "句点", this::commitNineKeyPeriod), "Period");
        for (Button key : java.util.List.of(comma, period)) {
            String face = key == comma ? commaFace : periodFace;
            // 全角「，」「。」的墨迹只占字身左下角，直接当键面文字会缩成贴底的小点。
            if ("，".equals(face) || "。".equals(face)) CenteredGlyphSpan.apply(key, face, 1.3f);
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
            addNineKey(marks, key);
        }
        lastRow.addView(marks, KeyboardGeometry.weightedMatchParentParams(1));
        grid.addView(lastRow, KeyboardGeometry.weightedWidthParams(1));
        container.addView(grid, KeyboardGeometry.weightedMatchParentParams(3));

        // 右列与网格的四行对齐：⌫、中/英、空格、换行。后三个是底栏那几个常驻键，连同它们的手势、图标和「确认」状态一起挪过来。
        LinearLayout actions = KeyboardGeometry.column(s);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, delete);
        adoptBarKey(actions, s.languageButton, KeyboardKeyRole.ACCENT);
        adoptBarKey(actions, s.spaceButton, KeyboardKeyRole.KEY);
        adoptBarKey(actions, s.enterButton, s.imeBottomRow.returnKeyRole());
        container.addView(actions, KeyboardGeometry.weightedMatchParentParams(0.8f));

        if (s.nineKeySpellingScroll != null && s.candidateViewport != null) {
            // 选择条只在创建键盘视图时建一次，拼音九键会把它挂进自己的侧栏；先从原来的父视图摘下再挂到候选行，否则 addView 会抛 IllegalStateException。挂在最后，盖在候选滚动区上面。
            if (s.nineKeySpellingScroll.getParent() instanceof android.view.ViewGroup previous)
                previous.removeView(s.nineKeySpellingScroll);
                s.candidateViewport.addView(s.nineKeySpellingScroll,
                    KeyboardGeometry.frameMatchParentParams());
            placeSpellingRow(true);
        }
    }

    /**
     * 选择条里那排按钮在两个位置的摆法：叠在候选行上（注音 9 键）时铺满整行高并垂直居中，否则按内容高度贴在行顶，下面空出一截；挂回拼音九键侧栏时恢复按内容高度、贴顶，侧栏第一格才是当前拼音。
     */
    private void placeSpellingRow(boolean candidateRow) {
        if (s.nineKeySpellings == null || s.nineKeySpellingScroll == null) return;
        // 两处的按钮样式不同：叠在候选行上时样式通道按候选按钮给了最小宽高和内边距，挂回侧栏后只重设颜色和字体，拼音按钮就一直是候选的尺寸。换位置时清空，renderNineKeySpellings 按当前位置重建。
        s.nineKeySpellings.removeAllViews();
        s.nineKeySpellingButtons.clear();
        s.nineKeySpellingScroll.setFillViewport(candidateRow);
        s.nineKeySpellings.setGravity(candidateRow ? Gravity.CENTER_VERTICAL : Gravity.NO_GRAVITY);
        if (s.nineKeySpellings.getLayoutParams() != null) {
            s.nineKeySpellings.getLayoutParams().height = candidateRow
                ? android.view.ViewGroup.LayoutParams.MATCH_PARENT
                : android.view.ViewGroup.LayoutParams.WRAP_CONTENT;
            s.nineKeySpellings.requestLayout();
        }
    }

    /**
     * 注音 9 键的一个音键：注音分组在上、数字在下，发送数字。
     *
     * <p>四个全宽注音在默认字号下比一个音键扣掉键距和内边距后的宽度还宽，第一行会折成两行、整个键面变成三行被裁。所以左右内边距收到 2dp，限定两行，并让整个键面在 7–14sp 之间等比缩到放得下（第二行一直是第一行的一半）。
     */
    private Button zhuyinNineKeySoundKey(ZhuyinNineKeyLayout.Key key) {
        String label = ZhuyinNineKeyLayout.accessibilityLabel(key);
        String face = ZhuyinNineKeyLayout.face(key);
        Button button = s.keyId(s.keyboardKey(face, label, () -> {
            if (s.connection != null) s.character(key.input(), false);
        }), ZhuyinNineKeyLayout.keyId(key));
        twoLineFace(button, face);
        int horizontal = s.pixels(2);
        ViewPolicy.setHorizontalPadding(button, horizontal);
        button.setMaxLines(2);
        ViewPolicy.setAutoSizeSp(button, 7, 14, 1);
        button.setContentDescription(label);
        if (button instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
        return button;
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
        LinearLayout options = KeyboardGeometry.row(s);
        int padding = s.pixels(5);
        ViewPolicy.setSymmetricPadding(options, padding, padding);
        GradientDrawable surface = DrawablePolicy.outlined(
            Color.parseColor(s.skin.background()), s.pixels(10),
            KeyboardGeometry.atLeastOnePixel(s, 1), Color.parseColor(s.skin.accent()));
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
            ViewPolicy.clearPadding(option);
            LinearLayout.LayoutParams params = KeyboardGeometry.linearParamsPx(
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
        java.util.List<String> kana = key.kana();
        StringBuilder result = new StringBuilder();
        result.append(kana.get(0)).append('\n');
        boolean first = true;
        int end = Math.min(5, kana.size());
        for (int index = 1; index < end; index++) {
            String label = kana.get(index);
            if (label.isEmpty()) continue;
            if (!first) result.append(' ');
            result.append(label);
            first = false;
        }
        return result.toString();
    }

    void inputJapaneseStroke(String input) {
        if (s.session == 0 || input.isEmpty()) return;
        for (int index = 0; index < input.length(); index++) s.character(input.charAt(index));
    }

    /** 九键的一个假名背后是一串罗马字（ち 是 chi），引擎的退格一次只删一个字母；接着删到读音末尾不再挂着半截罗马字，按一次就删掉一整个假名。 */
    void deleteJapaneseKana() {
        resetJapaneseToggle();
        if (s.connection == null) return;
        if (!s.command(0)) {
            s.deleteCodePointBeforeCursor();
            return;
        }
        for (int extra = 1; extra < JapaneseNineKeyLayout.LONGEST_STROKE && s.view != null
                && JapaneseNineKeyLayout.endsWithPendingRomaji(s.view.optString("reading", "")); extra++) {
            if (!s.command(0)) return;
        }
    }

    // ---- toggle input (トグル入力) ----
    private JapaneseNineKeyLayout.Key toggleKey;
    private int toggleDirection;
    private long toggleAt;
    private String toggleEditing = "";
    private String toggleLiteral = "";

    void resetJapaneseToggle() {
        toggleKey = null;
    }

    /** 上一次连点留下的假名还原样在那里：组字没被别的输入改过，或标点仍是光标前那一个字。中间打过别的字、删过、选过候选，都从新的一个假名开始。时间窗只管「再点同一个键算不算接着切换」（`withinWindow`）；→ 是明确的意图，不受它限制。 */
    private boolean japaneseToggleCurrent(boolean withinWindow) {
        if (toggleKey == null || s.connection == null || s.view == null) return false;
        if (withinWindow && android.os.SystemClock.uptimeMillis() - toggleAt
                > JapaneseNineKeyLayout.TOGGLE_WINDOW_MS) return false;
        String editing = s.view.optString("editing_text", "");
        if (toggleLiteral.isEmpty()) return editing.equals(toggleEditing);
        CharSequence before = s.connection.getTextBeforeCursor(toggleLiteral.length(), 0);
        return editing.isEmpty() && before != null && toggleLiteral.contentEquals(before);
    }

    private void recordJapaneseToggle(JapaneseNineKeyLayout.Key key, int direction) {
        toggleKey = key;
        toggleDirection = direction;
        toggleAt = android.os.SystemClock.uptimeMillis();
        toggleEditing = s.view == null ? "" : s.view.optString("editing_text", "");
        toggleLiteral = "";
        if (key.strokes().get(direction).isEmpty() && s.connection != null) {
            CharSequence before = s.connection.getTextBeforeCursor(1, 0);
            toggleLiteral = before == null ? "" : before.toString();
        }
    }

    /** 轻点假名键：同一个键在 {@link JapaneseNineKeyLayout#TOGGLE_WINDOW_MS} 内再点，就把刚打的假名换成下一个（あ→い→う…），否则照常打键面上的假名。数字符号层没有连点切换。 */
    void tapJapaneseKey(JapaneseNineKeyLayout.Key key) {
        if (s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS) {
            resetJapaneseToggle();
            selectJapaneseKey(key, 0);
            return;
        }
        if (key == toggleKey && japaneseToggleCurrent(true)) {
            stepJapaneseToggle(1);
            return;
        }
        resetJapaneseToggle();
        selectJapaneseKey(key, 0);
        recordJapaneseToggle(key, 0);
    }

    /** 撤掉上一次连点打出的假名（组字里删掉它的罗马字，标点删掉光标前那个字），换成循环里后 `step` 位的那个。 */
    private void stepJapaneseToggle(int step) {
        JapaneseNineKeyLayout.Key key = toggleKey;
        int next = JapaneseNineKeyLayout.toggleStep(JapaneseNineKeyLayout.toggleCycle(key), toggleDirection, step);
        String stroke = key.strokes().get(toggleDirection);
        if (stroke.isEmpty()) {
            s.deleteCodePointBeforeCursor();
        } else {
            for (int index = 0; index < stroke.length(); index++) {
                if (!s.command(0)) break;
            }
        }
        selectJapaneseKey(key, next);
        recordJapaneseToggle(key, next);
    }

    /** ◀：结束连点切换；没有组字时把光标左移一格。组字里的光标按罗马字移动，会停在半个假名里，所以组字时不动。 */
    void moveJapaneseCaretLeft() {
        resetJapaneseToggle();
        if (s.connection == null) return;
        if (s.view == null || s.view.optString("editing_text", "").isEmpty())
            s.sendDownUpKeyEvents(android.view.KeyEvent.KEYCODE_DPAD_LEFT);
    }

    /** →：结束连点切换，下一次轻点同一个键打新的假名（ああ）；不在切换、也没有组字时把光标右移一格。 */
    void advanceJapaneseToggle() {
        boolean toggling = japaneseToggleCurrent(false);
        resetJapaneseToggle();
        if (toggling || s.connection == null) return;
        if (s.view == null || s.view.optString("editing_text", "").isEmpty())
            s.sendDownUpKeyEvents(android.view.KeyEvent.KEYCODE_DPAD_RIGHT);
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
        final boolean[] previewShown = new boolean[1];
        // 轻点只输入键面上的假名，不弹十字预览；按住到系统长按时长，或手指已经滑出方向，才显示它。
        Runnable holdPreview = () -> {
            previewShown[0] = true;
            showJapaneseFlickPreview(button, key, direction[0]);
        };
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    direction[0] = 0;
                    previewShown[0] = false;
                    button.setPressed(true);
                    button.removeCallbacks(holdPreview);
                    button.postDelayed(holdPreview, ViewConfiguration.getLongPressTimeout());
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    direction[0] = JapaneseNineKeyLayout.direction(
                        event.getX() - origin[0], event.getY() - origin[1], s.pixels(12));
                    if (previewShown[0] || direction[0] != 0) {
                        button.removeCallbacks(holdPreview);
                        previewShown[0] = true;
                        showJapaneseFlickPreview(button, key, direction[0]);
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    button.setPressed(false);
                    button.removeCallbacks(holdPreview);
                    hideJapaneseFlickPreview();
                    if (direction[0] == 0) button.performClick();
                    else {
                        s.imeKeyFeedback.playFeedback(button);
                        s.countKey(button);
                        resetJapaneseToggle();
                        selectJapaneseKey(key, direction[0]);
                    }
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    button.setPressed(false);
                    button.removeCallbacks(holdPreview);
                    hideJapaneseFlickPreview();
                    return true;
                }
                default -> {
                    return true;
                }
            }
        });
    }

    /**
     * 两行键面（日语的 か / きくけこ、笔画的 一 / 横）：第二行是说明，和第一行同字号时两行装不进一行键高，下半截被裁掉。第二行缩到一半，并去掉上下内边距和字体留白。
     */
    static void twoLineFace(Button button, String label) {
        int lineBreak = label.indexOf('\n');
        if (lineBreak < 0) {
            button.setText(label);
            return;
        }
        android.text.SpannableString spanned = new android.text.SpannableString(label);
        spanned.setSpan(new android.text.style.RelativeSizeSpan(0.5f), lineBreak + 1, label.length(),
            android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        ViewPolicy.clearFontAndVerticalPadding(button);
        button.setText(spanned);
    }

    Button japaneseKey(JapaneseNineKeyLayout.Key key) {
        String description = key.kana().stream().filter(label -> !label.isEmpty())
            .collect(java.util.stream.Collectors.joining("、"));
        Button button = s.keyboardKey(japaneseKeyLabel(key), description,
            () -> tapJapaneseKey(key));
        twoLineFace(button, japaneseKeyLabel(key));
        button.setContentDescription("轻点输入" + key.kana().get(0)
            + "，连续轻点依次切换；左、上、右、下滑动选择其他假名");
        bindJapaneseFlick(button, key);
        if (button instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
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
        if (variants instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
        // 符号层上轻点直接输入第一个括号，长按才弹出全部括号；假名层不接长按，松手照常切换变体。
        bindFeedbackAction(variants, () -> {
            if (s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS) {
                commitNineKeyLiteral(JapaneseNineKeyLayout.digitBrackets().get(0));
            } else {
                s.command(MSIMEInputService.CYCLE_KANA_VARIANT_COMMAND);
            }
        });
        variants.setOnLongClickListener(ignored -> {
            if (s.keyboardLayer != KeyboardLayout.Layer.SYMBOLS) return false;
            s.imeKeyFeedback.playFeedback(variants);
            showJapaneseBracketOptions(variants);
            return true;
        });
        return variants;
    }

    private void bindFeedbackAction(Button button, Runnable action) {
        button.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
    }

    void addJapaneseSideKey(LinearLayout column, Button button, float weight) {
        // 侧列是功能键（123、☺（工具栏没有表情按钮时）、英、切换、⌫、空白）。角色不显式给时由描述推导，假名键的描述被判成功能面、侧列反倒成了字母面，整块配色主次颠倒。回车的角色由 updateReturnKey 跟着组字状态改。
        if (button instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        column.addView(button, KeyboardGeometry.weightedWidthParams(weight));
    }

    void rebuildJapaneseNineKeyRows() {
        LinearLayout container = KeyboardGeometry.row(s);
        // 日语九键没有底栏，四行（あ行到わ行加 小゛゜ 那一行）都在这一块里：按三行算高度时每行只剩三十来 dp，假名被裁掉下半截，底栏的位置又空着。
        s.imeStyler.adjustBottomRowBlockHeight(container);
        s.keyRows.addView(container, KeyboardGeometry.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3 + KeyboardGeometry.STANDARD_ROW_HEIGHT_DP)));

        LinearLayout modeColumn = KeyboardGeometry.column(s);
        s.japaneseSymbolsKey = s.keyId(s.keyboardKey("123", "切换到数字和符号", () -> {
            s.keyboardLayer = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                ? KeyboardLayout.Layer.LETTERS : KeyboardLayout.Layer.SYMBOLS;
            s.imeLetterRows.rebuildKeyRows();
            s.render();
        }), "SoftLayer");
        // 左列：◀ 光标左移、→ 结束连点切换（或光标右移），再是 123、☺（工具栏没有表情时）、英、切换，各占一格。
        addJapaneseSideKey(modeColumn, s.iconKey(KeyboardIconKey.Kind.CURSOR_LEFT, "◀",
            "光标左移", this::moveJapaneseCaretLeft), 1);
        addJapaneseSideKey(modeColumn, s.iconKey(KeyboardIconKey.Kind.TOGGLE_NEXT, "→",
            "结束连点切换，开始下一个假名；没有组字时光标右移", this::advanceJapaneseToggle), 1);
        addJapaneseSideKey(modeColumn, s.japaneseSymbolsKey, 1);
        boolean emojiKey = s.japaneseSideEmojiKey();
        if (emojiKey) {
            addJapaneseSideKey(modeColumn, s.keyId(s.keyboardKey("☺", "打开表情浏览", s.imePanels::showEmojiPicker),
                "SoftEmoji"), 1);
        }
        Button language = s.keyId(s.keyboardKey("英", "切换到英文输入", s::toggleInputLanguage),
            "SoftLanguage");
        addJapaneseSideKey(modeColumn, language, 1);
        if (s.offersGlobeKey()) {
            addJapaneseSideKey(modeColumn, s.keyId(s.keyboardKey("切换", "切换到下一个输入法",
                s::switchToNextInputMethodAfterCommit), "SoftGlobe"), 1);
        }
        container.addView(modeColumn, KeyboardGeometry.weightedMatchParentParams(0.17f));

        LinearLayout grid = KeyboardGeometry.column(s);
        java.util.List<JapaneseNineKeyLayout.Key> keys = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
            ? JapaneseNineKeyLayout.digitKeys() : JapaneseNineKeyLayout.keys();
        for (int rowIndex = 0; rowIndex < 4; rowIndex++) {
            LinearLayout row = KeyboardGeometry.row(s);
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
            grid.addView(row, KeyboardGeometry.weightedWidthParams(1));
        }
        container.addView(grid, KeyboardGeometry.weightedMatchParentParams(0.64f));

        LinearLayout side = KeyboardGeometry.column(s);
        Runnable deleteAction = this::deleteJapaneseKana;
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        addJapaneseSideKey(side, delete, 1);
        s.japaneseSpaceKey = s.keyId(s.keyboardKey("空白", "空白；左右滑动移动光标", s::space), "Space");
        s.imeBottomRow.bindSpaceCursor(s.japaneseSpaceKey);
        addJapaneseSideKey(side, s.japaneseSpaceKey, 1);
        s.japaneseReturnKey = s.keyId(s.keyboardKey("改行", "改行", s::enter), "Enter");
        addJapaneseSideKey(side, s.japaneseReturnKey, 2);
        container.addView(side, KeyboardGeometry.weightedMatchParentParams(0.19f));
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
        int layout = s.displayedTouchLayout(s.view);
        // 注音 9 键的选择条列出当前要钉住的那个音节的各个读音（ㄋㄧˇ、ㄌㄧˇ），拼音九键的列出拼音。
        boolean zhuyin = layout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT;
        boolean visible = (layout == MSIMEInputService.QUANPIN_NINE_KEY_LAYOUT || zhuyin)
            && spellings != null && spellings.length() > 0;
        if (visible) ViewPolicy.show(s.nineKeySpellingScroll);
        else ViewPolicy.hide(s.nineKeySpellingScroll);
        s.nineKeySpellingScroll.setContentDescription(zhuyin ? "注音读音选择" : "九键拼音选择");
        if (!visible) {
            s.nineKeySpellingIndices = java.util.List.of();
            s.nineKeySpellingGeneration = -1;
            for (Button key : s.nineKeySpellingButtons) ViewPolicy.hide(key);
            return;
        }
        s.nineKeySpellingGeneration = CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1);
        java.util.List<String> values = new java.util.ArrayList<>(spellings.length());
        java.util.List<Integer> indices = new java.util.ArrayList<>(spellings.length());
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
            if (slotVisible) ViewPolicy.show(key);
            else ViewPolicy.hide(key);
            if (slotVisible) {
                String spelling = values.get(slot);
                key.setText(spelling);
                key.setContentDescription((zhuyin ? "选择读音 " : "选择拼音 ") + spelling);
            }
        }
    }
}
