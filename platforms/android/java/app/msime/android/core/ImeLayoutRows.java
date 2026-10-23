package app.msime.android;

import app.msime.android.LayoutPolicy;


import app.msime.android.core.InputViewValuePolicy;
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
import org.json.JSONObject;

/** 26 键以外的键区：手写、九键、笔画、注音 9 键、日语九键（含 flick 与长按选项）、侧栏与九键拼音 / 注音读音选择；从 MSIMEInputService 原样搬出。 */
final class ImeLayoutRows {
    private final MSIMEInputService s;
    /** 拼音九键侧栏里的标点列；拼音列出现时让出位置。换了布局后它已不在侧栏里，按父视图判断。 */
    private View nineKeyPunctuation;
    private Button letterOptionsAnchor;
    private LinearLayout letterOptions;
    private final java.util.List<Button> letterOptionButtons = new java.util.ArrayList<>();
    private int letterOptionIndex = -1;
    private boolean letterOptionsEntered;

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
        resetForInput();
        pinyinWorker.shutdownNow();
    }

    /** 输入目标或键盘视图结束时调用：不让旧输入框的停笔任务和提示写回新面板。 */
    void resetForInput() {
        cancelPendingInk();
        cancelHandwritingNotice();
        pinyinGeneration++;
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
        KeyboardTypographyPolicy.setKeyTextSize(s.handwritingStatus, 13);
        s.handwritingStatus.setContentDescription("手写状态");
        ViewPolicy.setNonInteractive(s.handwritingStatus);

        LinearLayout row = LayoutPolicy.row(s);

        LinearLayout punctuation = LayoutPolicy.column(s);
        for (String symbol : NineKeyLayout.punctuation()) {
            Button key = s.keyId(s.keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            // 全角「，」「。」的墨迹只占字身左下角，直接当键面文字会缩成贴底的小点。
            if ("，".equals(symbol) || "。".equals(symbol)) CenteredGlyphSpan.apply(key, symbol, 1.3f);
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
            addNineKey(punctuation, key);
        }
        row.addView(punctuation, LayoutPolicy.weightedMatchParentParams(0.7f));

        FrameLayout area = new FrameLayout(s);
        s.handwritingCanvas = new HandwritingCanvas(s);
        s.handwritingCanvas.applySkin(s.handwritingSkin);
        s.handwritingCanvas.setInk(handwritingPreferences.inkColor(),
            s.pixels(handwritingPreferences.strokeWidth()));
        area.addView(s.handwritingCanvas, LayoutPolicy.frameMatchParentParams());

        FrameLayout cardFrame = new FrameLayout(s);
        ViewPolicy.setNonInteractive(cardFrame);
        FrameLayout.LayoutParams cardParams = LayoutPolicy.frameMatchParentParams();
        int inset = s.pixels(3);
        cardParams.setMargins(inset, inset, inset, inset);
        area.addView(cardFrame, cardParams);
        cardFrame.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> s.handwritingCanvas.setCardRect(
                    left, top, right, bottom));
        cardFrame.addView(s.handwritingStatus, LayoutPolicy.frameMatchWidthWrapParams(
            Gravity.CENTER));

        s.handwritingDownload = new Button(s);
        ViewPolicy.setAllCapsFalse(s.handwritingDownload);
        KeyboardTypographyPolicy.setKeyTextSize(s.handwritingDownload, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
        ViewPolicy.setSingleLine(s.handwritingDownload);
        bindFeedbackAction(s.handwritingDownload, s::downloadHandwritingModel);
        FrameLayout.LayoutParams downloadParams = LayoutPolicy.frameMatchWidthHeightPx(s.pixels(48));
        downloadParams.gravity = Gravity.CENTER;
        downloadParams.leftMargin = s.pixels(16);
        downloadParams.rightMargin = s.pixels(16);
        cardFrame.addView(s.handwritingDownload, downloadParams);
        row.addView(area, LayoutPolicy.weightedMatchParentParams(3f));

        LinearLayout tools = LayoutPolicy.column(s);
        Button delete = s.keyId(s.backspaceKey(s::deleteFromHandwriting), "Backspace");
        // 和其他布局的删除键一样按住加速连删、上滑快速删除（#5585）；有墨迹时每次删一笔。
        s.imeLetterRows.bindBackspaceRepeat(delete, s::deleteFromHandwriting);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(tools, delete);
        Button rewrite = s.keyboardKey("重写", "清空手写", () -> {
            cancelPendingInk();
            s.clearHandwriting();
        });
        if (rewrite instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(tools, rewrite);
        row.addView(tools, LayoutPolicy.weightedMatchParentParams(0.8f));
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
                handwritingPreferences.mode() == HandwritingPreferences.Mode.LINE
                        ? HandwritingRecognizer.preContext(s.connection) : "");
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
            JSONObject value = JsonPolicy.objectValue(root);
            JSONArray entries = value == null ? null : value.optJSONArray("entries");
            JSONObject entry = entries == null || entries.length() == 0 ? null : entries.optJSONObject(0);
            return entry == null ? ""
                : TextPolicy.trimmed(JsonPolicy.strictStringOrEmpty(entry.opt("key"))
                    .replace('\'', ' '));
        } catch (JSONException | RuntimeException | LinkageError error) {
            return "";
        }
    }

    void addNineKey(LinearLayout parent, Button key) {
        boolean horizontal = parent.getOrientation() == LinearLayout.HORIZONTAL;
        parent.addView(key, LayoutPolicy.weightedCrossAxisFillParams(horizontal, 1));
    }

    /** 无键帽符号栏保留底轨；独立键帽下不再叠加整列背景。 */
    void applySidebarRail() {
        if (s.nineKeySidebar == null) return;
        if (builtSidebarCapped) {
            ViewPolicy.clearBackground(s.nineKeySidebar);
            return;
        }
        GradientDrawable rail = DrawablePolicy.rounded(
            Color.parseColor(s.skin.sidebarBackground()), s.pixels(s.skin.cornerRadius()));
        ViewPolicy.setBackground(s.nineKeySidebar, rail);
    }

    /** 当前键行建出来时用的左侧符号和它属于哪个键面；设置改了以后 {@link #sidebarStale} 据此判断要不要重建。 */
    private java.util.List<String> builtSidebarSymbols;
    private boolean builtSidebarDigits;
    private boolean builtSidebarCapped;

    /** 本地设置里的左侧符号（不合规时是默认的那张表）：字母键面一张，拼音九键的数字键面另一张。 */
    java.util.List<String> sidebarSymbols(boolean digits) {
        return NineKeySidebarPolicy.sidebarSymbols(digits, s.localSettings.text(AndroidLocalSettings.NINE_KEY_SYMBOLS),
            s.localSettings.text(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS));
    }

    /** 九键、14 键或笔画的符号栏与设置不一致时，下一次渲染重建键行。 */
    boolean sidebarStale() {
        return s.nineKeySidebar != null && builtSidebarSymbols != null
            && !builtSidebarSymbols.equals(sidebarSymbols(builtSidebarDigits));
    }

    /**
     * 九键、14 键与笔画键盘左侧的符号栏：数字键面默认一屏四项，笔画三项，其他键面最多五项，更多的上下滚动。九键与笔画使用独立键帽，14 键共用一条底轨；点按原样上屏（全角模式下转全角），完整符号对的光标停在中间。
     */
    private FrameLayout symbolSidebar(boolean digits, boolean capped) {
        return symbolSidebar(digits, capped,
            digits ? NineKeySidebarPolicy.DIGIT_VISIBLE_ROWS : NineKeySidebarPolicy.VISIBLE_ROWS);
    }

    private FrameLayout symbolSidebar(boolean digits, boolean capped, int visibleRows) {
        java.util.List<String> symbols = sidebarSymbols(digits);
        NineKeySymbolRail rail = new NineKeySymbolRail(s, visibleRows);
        rail.setContentDescription("符号栏，可上下滑动");
        for (String symbol : symbols) {
            Button key = s.keyId(s.keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            // 「……」这类两个字宽的符号在窄栏里不折行。
            ViewPolicy.setSingleLine(key);
            if (capped)
                KeyboardTypographyPolicy.setKeyTextSize(key, KeyboardTypographyPolicy.NINE_KEY_SIDE_TEXT_SP);
            // 全角「，」「。」的墨迹只占字身左下角，直接当键面文字会缩成贴底的小点。
            if ("，".equals(symbol) || "。".equals(symbol)) CenteredGlyphSpan.apply(key, symbol, 1.3f);
            // 独立键帽与无键帽底轨按布局区分，避免按钮和整列背景叠加。
            if (key instanceof KeyboardPressButton press)
                press.setKeyboardRole(capped ? KeyboardKeyRole.KEY : KeyboardKeyRole.PLAIN);
            rail.addSymbol(key);
        }
        FrameLayout sidebar = new FrameLayout(s);
        s.nineKeySidebar = sidebar;
        builtSidebarSymbols = symbols;
        builtSidebarDigits = digits;
        builtSidebarCapped = capped;
        applySidebarRail();
        sidebar.addView(rail, LayoutPolicy.frameMatchParentParams());
        // 拼音九键的拼音列挂进同一个侧栏时盖住整条符号栏，renderNineKeySpellings 据此让出符号栏；笔画键盘不挂拼音列，父容器对不上，不受影响。
        nineKeyPunctuation = rail;
        return sidebar;
    }

    /** 九宫格建出来时是否画着九键双拼、按的是哪些键面；没画九键双拼时为 null。拼音九键和九键双拼是同一个触屏布局，布局编号不变时要靠它判断键面过期（{@link #shuangpinStale}）。 */
    private java.util.List<NineKeyShuangpinKeypad.Face> builtShuangpinFaces;
    /** 当前键行是 {@link #rebuildNineKeyRows} 建的。 */
    private boolean builtNineKeyGrid;

    /** 按实际绘制的网格选择高度分摊，包含其他中文布局借用的数字页。 */
    boolean nineKeyGridDrawn() { return builtNineKeyGrid; }

    /** 键行换成别的布局时调用。 */
    void forgetNineKeyGrid() {
        builtNineKeyGrid = false;
        builtShuangpinFaces = null;
    }

    /** 九宫格的键面过期了：拼音九键和九键双拼之间换了（布局编号相同，`apply` 不会重建），或者九键双拼换了表。 */
    boolean shuangpinStale() {
        if (!builtNineKeyGrid) return false;
        boolean drawn = s.keyboardLayer == KeyboardLayout.Layer.LETTERS && s.nineKeyShuangpinDrawn();
        if (drawn != (builtShuangpinFaces != null)) return true;
        return drawn && !s.nineKeyShuangpinFaces().equals(builtShuangpinFaces);
    }

    void rebuildNineKeyRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = LayoutPolicy.row(s);
        container.setTag(MSIMEInputService.KeyboardHeightRole.nineKeyGrid());
        s.keyRows.addView(container, LayoutPolicy.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3)));

        boolean digits = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
        boolean borrowed = s.displayedTouchLayout(s.view) != MSIMEInputService.QUANPIN_NINE_KEY_LAYOUT;
        // 左列符号可滚动和自定义，数字层左列底部另有 #+= 入口；数字层右列是删除、小数点和带语音手势的空格，0 移到底行中间（借用数字页同样如此）。
        FrameLayout sidebar = symbolSidebar(digits, true);
        attachSpellings(sidebar, SpellingPlacement.SIDEBAR);
        if (digits) {
            LinearLayout left = LayoutPolicy.column(s);
            left.addView(sidebar, LayoutPolicy.weightedWidthParams(1));
            Button more = s.keyId(s.keyboardKey("#+=", "切换到更多符号", s.imeLetterRows::showMoreSymbols), "SoftLayer");
            KeyboardTypographyPolicy.setKeyTextSize(more, 14);
            left.addView(more, LayoutPolicy.matchWidthHeightPx(s.pixels(40)));
            container.addView(left, LayoutPolicy.weightedMatchParentParams(NineKeyLayout.SIDE_COLUMN_WEIGHT));
        } else container.addView(sidebar, LayoutPolicy.weightedMatchParentParams(NineKeyLayout.SIDE_COLUMN_WEIGHT));

        LinearLayout grid = LayoutPolicy.column(s);
        boolean composing = s.hasEngineComposition();
        // 九键双拼：字母键面换成表里的声母和韵母，见 addShuangpinGrid；数字键面与拼音九键相同。
        boolean shuangpin = !digits && s.nineKeyShuangpinDrawn();
        builtNineKeyGrid = true;
        builtShuangpinFaces = shuangpin ? s.nineKeyShuangpinFaces() : null;
        if (shuangpin) addShuangpinGrid(grid, composing);
        for (java.util.List<NineKeyLayout.Key> keys : shuangpin ? java.util.List.<java.util.List<NineKeyLayout.Key>>of() : NineKeyLayout.rows()) {
            LinearLayout row = LayoutPolicy.row(s);
            for (NineKeyLayout.Key key : keys) {
                String description = NineKeyLayout.description(key, digits, composing);
                // 数字层按字面上屏，不把数字交给拼音组码或选词。
                NineKeyDigitButton keyButton = s.nineKeyGridKey(
                    NineKeyLayout.face(key, digits, composing), description,
                    digits ? () -> commitNineKeyLiteral(NineKeyLayout.digitInput(key))
                        : NineKeyLayout.offersPunctuation(key, false) ? () -> symbolKey(key)
                        : () -> s.character(key.input()));
                KeyboardTypographyPolicy.setFittedKeyTextSize(keyButton, 14,
                    digits ? 22 : KeyboardTypographyPolicy.NINE_KEY_LABEL_TEXT_SP);
                if (NineKeyLayout.offersPunctuation(key, digits)) s.nineKeySymbolKey = keyButton;
                // 常规键面借用网格时仍记普通数字键，避免统计页凭空多出拼音九键。
                s.keyId(keyButton, borrowed ? KeyPressIds.forCharacter(NineKeyLayout.digitInput(key).charAt(0))
                    : KeyPressIds.forNineKeyDigit(key.digit()));
                // 九格都印长按可输入的数字；1 短按是常用标点（放进候选条）或分词，不把 1 送进拼音引擎。
                keyButton.setDigitText(digits ? "" : NineKeyLayout.digitInput(key));
                if (!digits) {
                    keyButton.setContentDescription("按键 " + description + "；长按输入 " + key.digit()
                        + (NineKeyLayout.offersPunctuation(key, false) ? "" : " 或字母"));
                    keyButton.setOnLongClickListener(ignored -> {
                        // 长按只算这格的一次按键，随后选择菜单不重复统计。
                        s.countKey(keyButton);
                        showNineKeyHoldOptions(keyButton, NineKeyLayout.digitInput(key),
                            NineKeyLayout.offersPunctuation(key, false) ? ""
                                : key.label().toLowerCase(java.util.Locale.ROOT));
                        return true;
                    });
                }
                if (!digits) bindNineKeySwipe(keyButton, key);
                addNineKey(row, keyButton);
            }
            grid.addView(row, LayoutPolicy.weightedWidthParams(1));
        }
        container.addView(grid, LayoutPolicy.weightedMatchParentParams(NineKeyLayout.GRID_WEIGHT));

        LinearLayout actions = LayoutPolicy.column(s);
        Runnable deleteAction = borrowed ? s::deleteFromHandwriting : () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        // 九键双拼的网格有四行，⌫ 占两格，右列仍与网格逐行对齐。
        if (shuangpin) actions.addView(delete, LayoutPolicy.weightedCrossAxisFillParams(false, 2));
        else addNineKey(actions, delete);
        if (digits) {
            // 小数点按字面上屏；移动原空格键保留点按、长按语音与光标手势，0 由底行提供。
            Button period = s.keyId(s.keyboardKey(".", "小数点", () -> commitNineKeyLiteral(".")), "Period");
            KeyboardTypographyPolicy.setKeyTextSize(period, 22);
            if (period instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
            addNineKey(actions, period);
            adoptBarKey(actions, s.spaceButton, KeyboardKeyRole.KEY);
            // 重建数字网格时空格换了父容器，即使底行签名相同也要重新确认归属。
            s.actionRowSignature = "";
            container.addView(actions, LayoutPolicy.weightedMatchParentParams(NineKeyLayout.SIDE_COLUMN_WEIGHT));
            return;
        }
        // 重输：组字时丢掉整串数字和选过的拼音，和搜狗、讯飞九键右列的「重输」在同一个位置。这一格曾经改成「拆分」、重输挪到长按 ⌫，用户照习惯按这里清空却得到一个音节分界，长按 ⌫ 也没人知道；拆分现在是组字时的 1 键「分词」（symbolKey）。长按 ⌫ 清空照旧保留。没在组字时什么也不发生。
        Button rewrite = s.keyboardKey("重输", "重新输入拼音", () -> {
            if (s.hasEngineComposition()) s.discardComposition();
        });
        KeyboardTypographyPolicy.setKeyTextSize(rewrite, KeyboardTypographyPolicy.NINE_KEY_ACTION_TEXT_SP);
        if (rewrite instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, rewrite);
        java.util.List<String> punctuation = NineKeyLayout.punctuation();
        String last = punctuation.get(punctuation.size() - 1);
        Button exclamation = s.keyId(s.keyboardKey(last, "符号 " + last,
            () -> commitNineKeyLiteral(last)), "SoftPunctuation");
        s.imeLetterRows.bindSuperscriptShortcut(exclamation, last);
        KeyboardTypographyPolicy.setKeyTextSize(exclamation, KeyboardTypographyPolicy.NINE_KEY_LABEL_TEXT_SP);
        if (exclamation instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, exclamation);
        container.addView(actions, LayoutPolicy.weightedMatchParentParams(NineKeyLayout.SIDE_COLUMN_WEIGHT));
    }

    /**
     * 九键双拼（#6572）的数字网格：四行，前三行是 1–9，最后一行是分词键、0 和逗号，与拼音九键同一个三行高的外框。
     *
     * <p>每个数字键以数字为主标签，附单行简短提示（{@link NineKeyShuangpinKeypad#compactHint}），点按把数字交给引擎；表没用到的键印着数字本身，点按什么也不送。拼音九键的 1 键是「@#」或「分词」，九键双拼的 1 常常放着韵母（米旮旯、辜氏都用它），所以「@#」和「分词」挪到最后一行最左边，它仍是 {@link MSIMEInputService#nineKeySymbolKey}，随组字状态换键面。0 在 8 的正下方：表用到 0 时它是组码键（米旮旯 3 的 u ü ei），否则原样上屏 0。键上的长按和滑动只属于拼音九键（它们给的是键上的字母），这里不绑。
     */
    private void addShuangpinGrid(LinearLayout grid, boolean composing) {
        java.util.List<NineKeyShuangpinKeypad.Face> faces = s.nineKeyShuangpinFaces();
        for (java.util.List<NineKeyLayout.Key> keys : NineKeyLayout.rows()) {
            LinearLayout row = LayoutPolicy.row(s);
            for (NineKeyLayout.Key key : keys) addNineKey(row, shuangpinKey(faces, (char) ('0' + key.digit())));
            grid.addView(row, LayoutPolicy.weightedWidthParams(1));
        }
        LinearLayout lastRow = LayoutPolicy.row(s);
        NineKeyLayout.Key symbols = NineKeyLayout.rows().get(0).get(0);
        NineKeyDigitButton separator = s.nineKeyGridKey(NineKeyLayout.face(symbols, false, composing),
            NineKeyLayout.description(symbols, false, composing), () -> symbolKey(symbols));
        s.nineKeySymbolKey = separator;
        s.keyId(separator, "SoftSymbol");
        addNineKey(lastRow, separator);
        addNineKey(lastRow, shuangpinKey(faces, '0'));
        boolean chinese = s.sendsChinesePunctuation();
        String commaFace = ChineseSymbolFaces.face(",", chinese);
        Button comma = s.keyId(s.keyboardKey(commaFace, "逗号", () -> {
            if (s.dedicatedEnglish || !s.punctuation(',')) commitNineKeyLiteral(",");
        }), KeyPressIds.forCharacter(','));
        if ("，".equals(commaFace)) CenteredGlyphSpan.apply(comma, commaFace, 1.3f);
        if (comma instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
        addNineKey(lastRow, comma);
        grid.addView(lastRow, LayoutPolicy.weightedWidthParams(1));
    }

    /** 九键双拼的一个数字键：数字作主标签，表用到它时附简短提示、点按送这个数字；没用到时只印数字，0 原样上屏，其他键不送。 */
    private NineKeyDigitButton shuangpinKey(java.util.List<NineKeyShuangpinKeypad.Face> faces, char digit) {
        NineKeyShuangpinKeypad.Face face = NineKeyShuangpinKeypad.face(faces, digit);
        String digitText = String.valueOf(digit);
        NineKeyDigitButton key;
        if (face != null) {
            // 数字独占主行，只保留一行简短提示，不再把完整声韵母表铺满键帽。
            key = s.nineKeyGridKey(digitText, NineKeyShuangpinKeypad.description(face),
                () -> s.character(digit));
            String hint = NineKeyShuangpinKeypad.compactHint(face);
            KeyboardTypographyPolicy.setKeyTextSize(key, 22);
            key.setHintText(hint);
        } else {
            key = s.nineKeyGridKey(digitText, "数字 " + digit,
                digit == '0' ? () -> commitNineKeyLiteral("0") : () -> {});
        }
        s.keyId(key, digit == '0' ? "Nine0" : KeyPressIds.forNineKeyDigit(digit - '0'));
        return key;
    }

    /**
     * 笔画键盘：九键外框（标点侧栏、⌫ 列、固定高度）中间换成 {@link StrokeKeyboardLayout} 的 2×3 笔画网格。
     *
     * <p>笔画键直接走 character()，不走 type()：type() 会套用 Shift 大小写，也会把 ASCII 标点交给标点路径。九键的拼音选择条只属于拼音九键，这里不挂（挂上去要先从旧侧栏摘下，否则 addView 会抛异常）。
     */
    void rebuildStrokeRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = LayoutPolicy.row(s);
        s.imeStyler.adjustThreeRowBlockHeight(container);
        s.keyRows.addView(container, LayoutPolicy.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3)));

        // 外框和拼音九键一样三行对齐：左列是和拼音九键同一张可滚动的符号表，！ 在右列最下；中间两行笔画下面再一行 @#、0、句点。原来左列四个、中间两行、右列三个，三列互不对齐，看起来像少了一行。
        java.util.List<String> symbols = NineKeyLayout.punctuation();
        FrameLayout sidebar = symbolSidebar(false, true, 3);
        container.addView(sidebar, LayoutPolicy.weightedMatchParentParams(0.9f));

        LinearLayout grid = LayoutPolicy.column(s);
        for (java.util.List<StrokeKeyboardLayout.Key> keys : StrokeKeyboardLayout.rows()) {
            LinearLayout row = LayoutPolicy.row(s);
            for (StrokeKeyboardLayout.Key key : keys) {
                Button keyButton = s.keyboardKey(StrokeKeyboardLayout.face(key),
                    StrokeKeyboardLayout.accessibilityLabel(key), () -> strokeKey(key));
                keyButton.setContentDescription(StrokeKeyboardLayout.accessibilityLabel(key));
                // 键面只画笔画符号；矮键或系统大字体时按键帽空间缩小，名称保留在无障碍描述里。
                KeyboardTypographyPolicy.setFittedKeyTextSize(keyButton, 14, 22);
                if (keyButton instanceof KeyboardPressButton press)
                    press.setKeyboardRole(KeyboardKeyRole.KEY);
                s.keyId(keyButton, KeyPressIds.forCharacter(key.input()));
                if (key.input() == StrokeKeyboardLayout.WILDCARD) s.strokeWildcardKey = keyButton;
                addNineKey(row, keyButton);
            }
            grid.addView(row, LayoutPolicy.weightedWidthParams(1));
        }
        LinearLayout lastRow = LayoutPolicy.row(s);
        Button symbolsKey = s.keyId(s.keyboardKey("@#", "符号", s.imePanels::showSymbolPanel), KeyPressIds.forNineKeyDigit(1));
        Button zero = s.keyId(s.keyboardKey("0", "数字 0", () -> commitNineKeyLiteral("0")), "Nine0");
        String periodFace = ChineseSymbolFaces.face(".", s.sendsChinesePunctuation());
        Button period = s.keyId(s.keyboardKey(periodFace, "句点", this::commitNineKeyPeriod), "Period");
        if ("。".equals(periodFace)) CenteredGlyphSpan.apply(period, periodFace, 1.3f);
        for (Button key : java.util.List.of(symbolsKey, zero, period)) {
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
            addNineKey(lastRow, key);
        }
        grid.addView(lastRow, LayoutPolicy.weightedWidthParams(1));
        container.addView(grid, LayoutPolicy.weightedMatchParentParams(3));

        LinearLayout actions = LayoutPolicy.column(s);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, delete);
        // 右列和拼音九键一样三等分（⌫、拆分、！）、与三行网格逐行对齐；原来 ⌫ 独占两行，看起来像别的键盘拼进来的。笔画不需要拆分，中间这格是「重输」：丢掉已打的整串笔画。
        Button rewrite = s.keyboardKey("重输", "重新输入笔画", () -> {
            if (s.hasEngineComposition()) s.discardComposition();
        });
        KeyboardTypographyPolicy.setKeyTextSize(rewrite, KeyboardTypographyPolicy.NINE_KEY_ACTION_TEXT_SP);
        if (rewrite instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, rewrite);
        String last = symbols.get(symbols.size() - 1);
        Button exclamation = s.keyId(s.keyboardKey(last, "符号 " + last,
            () -> commitNineKeyLiteral(last)), "SoftPunctuation");
        if (exclamation instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, exclamation);
        container.addView(actions, LayoutPolicy.weightedMatchParentParams(0.9f));
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
        if (role == KeyboardKeyRole.ACCENT) KeyboardTypographyPolicy.setKeyTextSize(key, 15);
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
        LinearLayout container = LayoutPolicy.row(s);
        // 和日语九键一样不要底栏，四行都在这一块里，总高度是三行九键加一条底栏，与其他九键键盘一样高：四行挤进三行高时键太扁。底栏的 123、中/英、空格、换行挪进两侧的列。
        s.imeStyler.adjustBottomRowBlockHeight(container);
        s.keyRows.addView(container, LayoutPolicy.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * KeyboardHeightPolicy.KEYBOARD_ROW_COUNT)));

        LinearLayout tones = LayoutPolicy.column(s);
        for (ZhuyinNineKeyLayout.Tone tone : ZhuyinNineKeyLayout.tones()) {
            String label = ZhuyinNineKeyLayout.accessibilityLabel(tone);
            Button key = s.keyId(s.keyboardKey(tone.face(), label, () -> {
                if (s.connection != null && ZhuyinInputPolicy.toneKeySends(
                        s.view == null ? "" : InputViewValuePolicy.editingText(s.view)))
                    s.character(tone.input(), false);
            }), tone.keyId());
            key.setContentDescription(label);
            // 声调符号本身只是一道短笔画，按默认字号画出来几乎看不见。
            KeyboardTypographyPolicy.setKeyTextSize(key, 20);
            CenteredGlyphSpan.apply(key, tone.face(), 1f);
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.KEY);
            addNineKey(tones, key);
        }
        container.addView(tones, LayoutPolicy.weightedMatchParentParams(0.7f));

        LinearLayout grid = LayoutPolicy.column(s);
        for (java.util.List<ZhuyinNineKeyLayout.Key> keys : ZhuyinNineKeyLayout.rows()) {
            LinearLayout row = LayoutPolicy.row(s);
            for (ZhuyinNineKeyLayout.Key key : keys) addNineKey(row, zhuyinNineKeySoundKey(key));
            grid.addView(row, LayoutPolicy.weightedWidthParams(1));
        }
        // 最后一行：123（数字与符号层，？！ 和其他符号都在那里）、0（仍在 8 的正下方）、逗号和句号各占半格。
        LinearLayout lastRow = LayoutPolicy.row(s);
        adoptBarKey(lastRow, s.localSettings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS) ? s.languageButton : s.layerButton, KeyboardKeyRole.ACCENT);
        addNineKey(lastRow, zhuyinNineKeySoundKey(ZhuyinNineKeyLayout.zero()));
        LinearLayout marks = LayoutPolicy.row(s);
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
        lastRow.addView(marks, LayoutPolicy.weightedMatchParentParams(1));
        grid.addView(lastRow, LayoutPolicy.weightedWidthParams(1));
        container.addView(grid, LayoutPolicy.weightedMatchParentParams(3));

        // 右列与网格的四行对齐：⌫、中/英、空格、换行。后三个是底栏那几个常驻键，连同它们的手势、图标和「确认」状态一起挪过来。
        LinearLayout actions = LayoutPolicy.column(s);
        Runnable deleteAction = () -> {
            if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
        };
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        if (delete instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        addNineKey(actions, delete);
        adoptBarKey(actions, s.localSettings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS) ? s.layerButton : s.languageButton, KeyboardKeyRole.ACCENT);
        adoptBarKey(actions, s.spaceButton, KeyboardKeyRole.KEY);
        adoptBarKey(actions, s.enterButton, s.imeBottomRow.returnKeyRole());
        container.addView(actions, LayoutPolicy.weightedMatchParentParams(0.8f));

        // 挂在最后，盖在候选滚动区上面。
        if (s.candidateViewport != null) attachSpellings(s.candidateViewport, SpellingPlacement.CANDIDATE_ROW);
    }

    /** 拼音选择只使用九键侧栏或注音九键候选行。 */
    enum SpellingPlacement { SIDEBAR, CANDIDATE_ROW }

    /** 14 键共用九键的左侧符号栏；组字时由纵向拼音列覆盖符号。 */
    FrameLayout fourteenSpellingSidebar() {
        FrameLayout sidebar = symbolSidebar(false, false);
        attachSpellings(sidebar, SpellingPlacement.SIDEBAR);
        return sidebar;
    }

    /** 换挂载位置时先摘下旧容器、清空按钮，避免父视图和按钮尺寸沿用另一种布局。 */
    private void attachSpellings(android.view.ViewGroup parent, SpellingPlacement placement) {
        if (s.nineKeySpellings == null || s.nineKeySpellingScroll == null) return;
        if (s.nineKeySpellingScroll.getParent() instanceof android.view.ViewGroup previous)
            previous.removeView(s.nineKeySpellingScroll);
        if (s.nineKeySpellings.getParent() instanceof android.view.ViewGroup holder)
            holder.removeView(s.nineKeySpellings);
        s.nineKeySpellings.removeAllViews();
        s.nineKeySpellingButtons.clear();
        boolean candidateRow = placement != SpellingPlacement.SIDEBAR;
        FrameLayout scroll;
        if (candidateRow) {
            android.widget.HorizontalScrollView row = new android.widget.HorizontalScrollView(s);
            row.setHorizontalScrollBarEnabled(false);
            row.setFillViewport(true);
            scroll = row;
        } else {
            android.widget.ScrollView column = new android.widget.ScrollView(s);
            column.setVerticalScrollBarEnabled(false);
            scroll = column;
        }
        s.nineKeySpellings.setOrientation(candidateRow ? LinearLayout.HORIZONTAL : LinearLayout.VERTICAL);
        ViewPolicy.setGravity(s.nineKeySpellings,
            candidateRow ? Gravity.CENTER_VERTICAL : Gravity.NO_GRAVITY);
        scroll.addView(s.nineKeySpellings, new FrameLayout.LayoutParams(
            candidateRow ? FrameLayout.LayoutParams.WRAP_CONTENT : FrameLayout.LayoutParams.MATCH_PARENT,
            candidateRow ? FrameLayout.LayoutParams.MATCH_PARENT : FrameLayout.LayoutParams.WRAP_CONTENT));
        ViewPolicy.hide(scroll);
        s.nineKeySpellingScroll = scroll;
        s.nineKeySpellingGeneration = -1;
        parent.addView(scroll, LayoutPolicy.frameMatchParentParams());
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
        ViewPolicy.setMaxLines(button, 2);
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

    /**
     * 拼音九键的 1 键：组字时是「分词」，把 ' 送给引擎，在已打的数字末尾定一个音节分界，只定在哪里断、不定是哪个拼音：94 分词 26 仍可以是 xi'an（西安）或 yi'an，但不再是 xian（先）；读音栏显示成 94'26，⌫ 先删分界再删数字。没在组字时是「@#」，把当前模式的常用标点放进候选条（{@link ImeNineKeyQuickPunctuation}），符号面板改从底行最左边的 符 打开。
     */
    private void symbolKey(NineKeyLayout.Key key) {
        if (NineKeyLayout.separatesSyllables(key, false, s.hasEngineComposition())) {
            s.character('\'', false);
        } else {
            s.imeNineKeyQuickPunctuation.show();
        }
    }

    /** 1 键的键面跟着组字状态在「@#」和「分词」之间换，render 时调用。 */
    void updateNineKeySymbolKey() {
        if (s.nineKeySymbolKey == null) return;
        boolean digits = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
        NineKeyLayout.Key key = NineKeyLayout.rows().get(0).get(0);
        boolean composing = s.hasEngineComposition();
        String face = NineKeyLayout.face(key, digits, composing);
        if (face.contentEquals(s.nineKeySymbolKey.getText())) return;
        s.nineKeySymbolKey.setText(face);
        s.nineKeySymbolKey.setContentDescription("按键 " + NineKeyLayout.description(key, digits, composing)
            + (digits || s.nineKeyShuangpinDrawn() ? "" : "；长按输入 1"));
    }

    void updateStrokeWildcardKey() {
        if (s.strokeWildcardKey == null) return;
        ViewPolicy.setEnabled(s.strokeWildcardKey,
            StrokeKeyboardLayout.sends(StrokeKeyboardLayout.WILDCARD, s.hasEngineComposition()));
    }

    /**
     * 拼音九键网格键的滑动（#5580，规则见 {@link NineKeySwipePolicy}）：沿「九键滑动输入数字」选的方向滑过「九键滑动距离」，键面换成数字、松手上屏这个数字；往反方向滑，弹出和长按一样的数字与字母选项。按下时不拦截，没滑过阈值时点按和长按照常由按钮自己处理；滑过阈值的那一刻给按钮补一个 CANCEL，它就不会再点按或长按。长按选项已经弹出时不再判定滑动。
     */
    private void bindNineKeySwipe(NineKeyDigitButton keyButton, NineKeyLayout.Key key) {
        final float[] downY = new float[1];
        final NineKeySwipePolicy.Gesture[] gesture = {NineKeySwipePolicy.Gesture.NONE};
        // 1 键的键面随组字状态在「@#」和「分词」之间换（updateNineKeySymbolKey），所以滑动开始时才记下当时的键面，松手后还原成它，而不是建键时的那个。
        final String[] face = {keyButton.getText().toString()};
        final String digit = NineKeyLayout.digitInput(key);
        final boolean hasLetters = TextPolicy.isDigit(key.input()) && key.label().length() > 1;
        Runnable restoreFace = () -> {
            keyButton.setText(face[0]);
            keyButton.setDigitText(digit);
        };
        keyButton.setOnTouchListener((view, event) -> {
            s.imeLetterRows.updateKeyPreview(keyButton, event);
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    downY[0] = DimensionPolicy.fromPixels(s, event.getY());
                    gesture[0] = NineKeySwipePolicy.Gesture.NONE;
                    return false;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (gesture[0] != NineKeySwipePolicy.Gesture.NONE) return true;
                    if (s.nineKeyHoldPopup != null) return false;
                    NineKeySwipePolicy.Gesture next = NineKeySwipePolicy.gesture(
                        s.localSettings.choice(AndroidLocalSettings.NINE_KEY_SWIPE), downY[0],
                        DimensionPolicy.fromPixels(s, event.getY()), hasLetters,
                        s.localSettings.integer(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE));
                    if (next == NineKeySwipePolicy.Gesture.NONE) return false;
                    gesture[0] = next;
                    MotionEvent cancel = MotionEvent.obtain(event);
                    cancel.setAction(MotionEvent.ACTION_CANCEL);
                    keyButton.onTouchEvent(cancel);
                    cancel.recycle();
                    if (next == NineKeySwipePolicy.Gesture.LETTERS) {
                        // 和长按一样：这一下是这个键的一次按压，之后从弹窗里选的不是另一次按键。
                        s.countKey(keyButton);
                        showNineKeyHoldOptions(keyButton, NineKeyLayout.digitInput(key),
                            key.label().toLowerCase(java.util.Locale.ROOT));
                    } else {
                        keyButton.setPressed(true);
                        face[0] = keyButton.getText().toString();
                        keyButton.setText(digit);
                        keyButton.setDigitText("");
                        s.imeLetterRows.updateKeyPreviewLabel(keyButton, digit);
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    NineKeySwipePolicy.Gesture done = gesture[0];
                    gesture[0] = NineKeySwipePolicy.Gesture.NONE;
                    if (done == NineKeySwipePolicy.Gesture.NONE) return false;
                    if (done == NineKeySwipePolicy.Gesture.DIGIT) {
                        keyButton.setPressed(false);
                        restoreFace.run();
                        s.imeKeyFeedback.playFeedback(keyButton);
                        s.countKey(keyButton);
                        commitNineKeyLiteral(digit);
                    }
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    if (gesture[0] == NineKeySwipePolicy.Gesture.DIGIT) {
                        keyButton.setPressed(false);
                        restoreFace.run();
                    }
                    gesture[0] = NineKeySwipePolicy.Gesture.NONE;
                    return false;
                }
                default -> { return gesture[0] != NineKeySwipePolicy.Gesture.NONE; }
            }
        });
    }

    /**
     * 长按弹出这一键上的字面字符，像 Apple 的长按弹窗：九键是印着的数字加小写字母，14 键只有这一组的两个字母（`digit` 为空）。选中后先结束组字再上屏（{@link #commitNineKeyHoldOption}）。
     */
    void showNineKeyHoldOptions(Button anchor, String digit, String letters) {
        dismissNineKeyHoldOptions();
        if (s.keyboardRoot == null) return;
        java.util.List<String> choices = new java.util.ArrayList<>(letters.length() + 1);
        if (!digit.isEmpty()) choices.add(digit);
        for (int index = 0; index < letters.length(); index++)
            choices.add(String.valueOf(letters.charAt(index)));
        showTextOptions(anchor, choices);
    }

    /** 中文字母的选项保留点选，并把原按键后续的移动用于拖选。 */
    void showLetterOptions(Button anchor, String input) {
        String lower = input.toLowerCase(java.util.Locale.ROOT);
        String upper = input.toUpperCase(java.util.Locale.ROOT);
        String hint = LetterHintTable.face(input, s.sendsChinesePunctuation(), s.punctuationMarks());
        java.util.List<String> choices = new java.util.ArrayList<>();
        choices.add(lower);
        if (!upper.equals(lower)) choices.add(upper);
        if (hint != null) choices.add(hint);
        LinearLayout options = showChoiceOptions(anchor, choices, choice -> {
            if (choice.equals(hint)) commitLetterHint(input);
            else commitNineKeyHoldOption(choice);
        });
        if (options != null) {
            letterOptionsAnchor = anchor;
            letterOptions = options;
            collectLetterOptionButtons(options);
        }
    }

    /** 弹窗以多行容器排版，拖选只跟踪其中实际可提交的按钮。 */
    private void collectLetterOptionButtons(android.view.ViewGroup group) {
        for (int index = 0; index < group.getChildCount(); index++) {
            View child = group.getChildAt(index);
            if (child instanceof Button button) letterOptionButtons.add(button);
            else if (child instanceof android.view.ViewGroup nested) collectLetterOptionButtons(nested);
        }
    }

    /** 弹窗可能贴着屏幕边缘移动，用每个选项布局后的屏幕坐标命中。 */
    void moveLetterOptions(Button anchor, MotionEvent event) {
        if (letterOptionsAnchor != anchor || letterOptions == null) return;
        int selected = -1;
        android.graphics.Rect bounds = new android.graphics.Rect();
        int[] location = new int[2];
        for (int index = 0; index < letterOptionButtons.size(); index++) {
            Button option = letterOptionButtons.get(index);
            option.getLocationOnScreen(location);
            bounds.set(location[0], location[1], location[0] + option.getWidth(), location[1] + option.getHeight());
            if (option.isShown() && bounds.contains((int) event.getRawX(), (int) event.getRawY())) {
                selected = index;
                break;
            }
        }
        // 长按后的轻微抖动不算拖选；进入过选项后，滑出才转为取消选择。
        letterOptionsEntered |= selected >= 0;
        if (selected == letterOptionIndex) return;
        letterOptionIndex = selected;
        for (int index = 0; index < letterOptionButtons.size(); index++) {
            Button option = letterOptionButtons.get(index);
            option.setSelected(index == selected);
            s.imeStyler.styleButton(option, false);
        }
    }

    /** 未进入选项的长按仍等下一次点选；进入过则在原手势内选中一次或关闭。 */
    boolean finishLetterOptions(Button anchor, MotionEvent event) {
        if (letterOptionsAnchor != anchor || letterOptions == null || !letterOptionsEntered) return false;
        moveLetterOptions(anchor, event);
        Button selected = letterOptionIndex < 0 ? null
            : letterOptionButtons.get(letterOptionIndex);
        if (selected == null) dismissNineKeyHoldOptions();
        else selected.performClick();
        return true;
    }

    void cancelLetterOptions(Button anchor) {
        if (letterOptionsAnchor == anchor) dismissNineKeyHoldOptions();
    }

    /** 直接滑出角标与选项中的符号共用 Engine 标点路由，保留组字转换、引号和成对补全。 */
    void commitLetterHint(String input) {
        dismissNineKeyHoldOptions();
        String hint = LetterHintTable.hint(input);
        if (hint == null) return;
        boolean engine = s.sendsChinesePunctuation() && LetterHintTable.routesThroughEngine(hint)
            && s.punctuation(hint.charAt(0));
        if (!engine) commitNineKeyLiteral(hint);
    }

    /** 123 符号页「！」键长按的上标、下标选项。 */
    void showSuperscriptOptions(Button anchor) {
        dismissNineKeyHoldOptions();
        if (s.keyboardRoot == null) return;
        showTextOptions(anchor, SuperscriptShortcutPolicy.variants());
    }

    /** 英文长按提供字符变体，O 另保留进入拼音键盘的动作。 */
    void showEnglishLetterOptions(Button anchor, String input) {
        showChoiceOptions(anchor, EnglishLetterHoldPolicy.choices(input), choice -> {
            if (EnglishLetterHoldPolicy.PINYIN_KEYBOARD.equals(choice)) {
                dismissNineKeyHoldOptions();
                s.toggleInputLanguage();
            } else commitNineKeyHoldOption(choice);
        });
    }

    /** 显示不依赖具体键位的短文本选项，并复用九键长按弹窗的生命周期。 */
    private void showTextOptions(Button anchor, java.util.List<String> choices) {
        showChoiceOptions(anchor, choices, this::commitNineKeyHoldOption);
    }

    private LinearLayout showChoiceOptions(Button anchor, java.util.List<String> choices,
            java.util.function.Consumer<String> action) {
        dismissNineKeyHoldOptions();
        s.imeLetterRows.hideKeyPreview();
        if (s.keyboardRoot == null || choices.isEmpty()) return null;
        s.imeKeyFeedback.playFeedback(anchor);
        LinearLayout options = LayoutPolicy.column(s);
        int padding = s.pixels(5);
        int cellWidth = s.pixels(36);
        int gap = s.pixels(2);
        int columns = Math.max(1, Math.min(5,
            (s.keyboardRoot.getWidth() - 2 * padding + gap) / (cellWidth + gap)));
        ViewPolicy.setSymmetricPadding(options, padding, padding);
        GradientDrawable surface = DrawablePolicy.outlined(
            Color.parseColor(s.skin.background()), s.pixels(10),
            DimensionPolicy.atLeastOnePixel(s, 1), Color.parseColor(s.skin.accent()));
        ViewPolicy.setBackground(options, surface);
        LinearLayout row = null;
        for (int index = 0; index < choices.size(); index++) {
            if (index % columns == 0) {
                row = LayoutPolicy.row(s);
                options.addView(row);
            }
            String choice = choices.get(index);
            Button option = s.keyboardKey(choice, "输入 " + choice,
                () -> action.accept(choice));
            option.setText(CandidateSymbolLabel.apply(s, choice, 0, choice.length()));
            option.setContentDescription("⮕".equals(choice) ? "切换到拼音键盘" : "输入 " + choice);
            ViewPolicy.clearPadding(option);
            LinearLayout.LayoutParams params = LayoutPolicy.linearParamsPx(
                s.pixels(36), s.pixels(38));
            if (row.getChildCount() > 0) params.setMarginStart(gap);
            row.addView(option, params);
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
        ViewPolicy.setElevation(popup, s.pixels(4));
        popup.setOnDismissListener(() -> {
            if (s.nineKeyHoldPopup == holder[0]) s.nineKeyHoldPopup = null;
            if (letterOptions == options) clearLetterOptions();
        });
        s.nineKeyHoldPopup = popup;
        int[] rootPosition = new int[2];
        int[] anchorPosition = new int[2];
        s.keyboardRoot.getLocationOnScreen(rootPosition);
        anchor.getLocationOnScreen(anchorPosition);
        int width = options.getMeasuredWidth();
        int height = options.getMeasuredHeight();
        int x = Math.max(rootPosition[0], Math.min(anchorPosition[0] + (anchor.getWidth() - width) / 2,
            rootPosition[0] + s.keyboardRoot.getWidth() - width));
        // API 28 子窗口仍受 IME 父窗口裁剪；空间不足时向键区内展开，保证每个选项可见可点。
        int y = Math.max(rootPosition[1], Math.min(anchorPosition[1] - height - s.pixels(6),
            rootPosition[1] + s.keyboardRoot.getHeight() - height));
        // showAtLocation 的坐标相对 IME 父窗口；按锚点换算偏移，避免把屏幕坐标重复加上窗口原点。
        popup.showAsDropDown(anchor, x - anchorPosition[0],
            y - anchorPosition[1] - anchor.getHeight());
        return options;
    }

    private void clearLetterOptions() {
        letterOptionsAnchor = null;
        letterOptions = null;
        letterOptionButtons.clear();
        letterOptionIndex = -1;
        letterOptionsEntered = false;
    }

    void commitNineKeyHoldOption(String text) {
        dismissNineKeyHoldOptions();
        if (s.connection == null) return;
        s.command(2);
        s.commitText(s.fullWidthOutput(text));
    }

    void dismissNineKeyHoldOptions() {
        clearLetterOptions();
        if (s.nineKeyHoldPopup != null) {
            s.nineKeyHoldPopup.dismiss();
            s.nineKeyHoldPopup = null;
        }
    }

    static String japaneseKeyLabel(JapaneseNineKeyLayout.Key key) {
        return key.kana().get(0);
    }

    void inputJapaneseStroke(String input) {
        if (s.session == 0 || input.isEmpty()) return;
        for (int index = 0; index < input.length(); index++) s.character(input.charAt(index));
    }

    /** 九键的一个假名背后是一串罗马字（ち 是 chi），引擎的退格一次只删一个字母；接着删到读音末尾不再挂着半截罗马字，按一次就删掉一整个假名。看的是画出来的读音 `display_reading`：な（`na`）删掉 `a` 剩下的 `n` 在那里还是字母，会接着删掉，而确定用的 `reading` 已把它当成ん。 */
    void deleteJapaneseKana() {
        resetJapaneseToggle();
        if (s.connection == null) return;
        if (!s.command(0)) {
            s.deleteCodePointBeforeCursor();
            return;
        }
        for (int extra = 1; extra < JapaneseNineKeyLayout.LONGEST_STROKE && s.view != null
                && JapaneseNineKeyLayout.endsWithPendingRomaji(InputViewValuePolicy.textOr(s.view, "display_reading",
                    InputViewValuePolicy.text(s.view, "reading"))); extra++) {
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
        if (withinWindow && !JapaneseNineKeyLayout.withinToggleWindow(
                android.os.SystemClock.uptimeMillis() - toggleAt)) return false;
        String editing = InputViewValuePolicy.editingText(s.view);
        if (toggleLiteral.isEmpty()) return editing.equals(toggleEditing);
        CharSequence before = InputConnectionGuard.textBeforeCursor(s.connection, toggleLiteral.length());
        return editing.isEmpty() && before != null && toggleLiteral.contentEquals(before);
    }

    private void recordJapaneseToggle(JapaneseNineKeyLayout.Key key, int direction) {
        toggleKey = key;
        toggleDirection = direction;
        toggleAt = android.os.SystemClock.uptimeMillis();
        toggleEditing = s.view == null ? "" : InputViewValuePolicy.editingText(s.view);
        toggleLiteral = "";
        if (key.strokes().get(direction).isEmpty() && s.connection != null) {
            CharSequence before = InputConnectionGuard.textBeforeCursor(s.connection, 1);
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
        if (s.view == null || InputViewValuePolicy.editingText(s.view).isEmpty())
            s.sendEditorKeyEvents(android.view.KeyEvent.KEYCODE_DPAD_LEFT);
    }

    /** →：结束连点切换，下一次轻点同一个键打新的假名（ああ）；不在切换、也没有组字时把光标右移一格。 */
    void advanceJapaneseToggle() {
        boolean toggling = japaneseToggleCurrent(false);
        resetJapaneseToggle();
        if (toggling || s.connection == null) return;
        if (s.view == null || InputViewValuePolicy.editingText(s.view).isEmpty())
            s.sendEditorKeyEvents(android.view.KeyEvent.KEYCODE_DPAD_RIGHT);
    }

    void selectJapaneseKey(JapaneseNineKeyLayout.Key key, int direction) {
        if (direction < 0 || direction >= key.kana().size()) return;
        if (key.kana().get(direction).isEmpty()) return;
        String stroke = key.strokes().get(direction);
        if (stroke.isEmpty()) commitNineKeyLiteral(key.kana().get(direction));
        else inputJapaneseStroke(stroke);
    }

    /** 上一次连点留下的键；组字已被别的输入改过时为 null，此后轻点从键面上的假名重新开始。时间窗由 {@link JapaneseNineKeyLayout#tapDirection} 判断。 */
    private JapaneseNineKeyLayout.Key currentJapaneseToggleKey() {
        return japaneseToggleCurrent(false) ? toggleKey : null;
    }

    /** 此刻轻点 `key` 会打出的方向（{@link JapaneseNineKeyLayout#tapDirection}）。按下时的提示中间格照它显示，免得提示写着 あ、松手却打出 い。 */
    int japaneseTapDirection(JapaneseNineKeyLayout.Key key) {
        return JapaneseNineKeyLayout.tapDirection(key, s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS,
            currentJapaneseToggleKey(), toggleDirection, android.os.SystemClock.uptimeMillis() - toggleAt);
    }

    /** `key` 的连点时间窗还剩多少毫秒，不在连点这个键时为 -1（{@link JapaneseNineKeyLayout#toggleRemainingMillis}）。 */
    long japaneseToggleRemainingMillis(JapaneseNineKeyLayout.Key key) {
        return JapaneseNineKeyLayout.toggleRemainingMillis(key, s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS,
            currentJapaneseToggleKey(), android.os.SystemClock.uptimeMillis() - toggleAt);
    }

    void showJapaneseFlickPreview(Button button, JapaneseNineKeyLayout.Key key, int direction, int tapDirection) {
        if (s.japaneseFlickPreview != null && s.keyboardRoot != null)
            s.japaneseFlickPreview.show(button, key, direction, tapDirection);
    }

    /** 不管提示属于哪个键都收起：键盘收起、换布局时用。 */
    void hideJapaneseFlickPreview() {
        if (s.japaneseFlickPreview != null) s.japaneseFlickPreview.hide();
    }

    /** 松开或取消 `button` 时用：提示此刻画的是 `button` 才收起。两只拇指交替按时，先按的键松开不会把后按、还按着的键的提示收掉。 */
    void hideJapaneseFlickPreview(Button button) {
        if (s.japaneseFlickPreview != null) s.japaneseFlickPreview.hideFor(button);
    }

    /** 提示此刻是否显示着（不论属于哪个键）；没有提示浮层时当作显示着，免得按住时每次移动都去重画。 */
    boolean japaneseFlickPreviewShown() {
        return s.japaneseFlickPreview == null || s.japaneseFlickPreview.showing();
    }

    /** 提示此刻是否画的是 `button`。 */
    boolean japaneseFlickPreviewShownFor(Button button) {
        return s.japaneseFlickPreview != null && s.japaneseFlickPreview.showingFor(button);
    }

    void bindJapaneseFlick(Button button, JapaneseNineKeyLayout.Key key) {
        final float[] origin = new float[2];
        final int[] direction = new int[1];
        // 按下就显示提示（#6716）：它只占被按的键和四周几 dp（JapaneseFlickGuideGeometry），不再盖住相邻的键，所以轻点也显示，不必像原来的大十字那样等到长按时长才出来。中间格是此刻轻点会打出的假名，每次画都重新算（另一只拇指在这期间打了字，连点就断了）；连点时间窗在按住期间到期时换回键面上的假名。
        Runnable toggleExpired = () -> {
            // 提示此刻画的是别的键（另一只拇指后按的）就不抢回来，这个键下一次移动时再画。
            if (japaneseFlickPreviewShownFor(button))
                showJapaneseFlickPreview(button, key, direction[0], japaneseTapDirection(key));
        };
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    direction[0] = 0;
                    button.setPressed(true);
                    button.removeCallbacks(toggleExpired);
                    long remaining = japaneseToggleRemainingMillis(key);
                    if (remaining >= 0) button.postDelayed(toggleExpired, remaining + 1);
                    showJapaneseFlickPreview(button, key, direction[0], japaneseTapDirection(key));
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    int next = JapaneseNineKeyLayout.direction(
                        event.getX() - origin[0], event.getY() - origin[1], s.pixels(12));
                    // 方向变了才重画；后按的键先松开、把提示收掉了时，还按着的这个键下一次移动就把自己的提示画回来。
                    if (next != direction[0] || !japaneseFlickPreviewShown()) {
                        direction[0] = next;
                        showJapaneseFlickPreview(button, key, direction[0], japaneseTapDirection(key));
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    button.setPressed(false);
                    button.removeCallbacks(toggleExpired);
                    hideJapaneseFlickPreview(button);
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
                    button.removeCallbacks(toggleExpired);
                    hideJapaneseFlickPreview(button);
                    return true;
                }
                default -> {
                    return true;
                }
            }
        });
    }

    /**
     * 两行键面：第二行是说明，和第一行同字号时两行装不进一行键高，下半截被裁掉。第二行缩到一半，并去掉上下内边距和字体留白。
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
        String label = japaneseKeyLabel(key);
        Button button = s.keyboardKey(label, description,
            () -> tapJapaneseKey(key));
        ViewPolicy.setSingleLine(button);
        KeyboardTypographyPolicy.setKeyTextSize(button, 22);
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
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
    }

    void addJapaneseSideKey(LinearLayout column, Button button, float weight) {
        // 侧列是功能键（123、☺（工具栏没有表情按钮时）、英、切换、⌫、空白）。角色不显式给时由描述推导，假名键的描述被判成功能面、侧列反倒成了字母面，整块配色主次颠倒。回车的角色由 updateReturnKey 跟着组字状态改。
        if (button instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        column.addView(button, LayoutPolicy.weightedWidthParams(weight));
    }

    void rebuildJapaneseNineKeyRows() {
        LinearLayout container = LayoutPolicy.row(s);
        // 日语九键没有底栏，四行（あ行到わ行加 小゛゜ 那一行）都在这一块里：按三行算高度时每行只剩三十来 dp，假名被裁掉下半截，底栏的位置又空着。
        s.imeStyler.adjustBottomRowBlockHeight(container);
        s.keyRows.addView(container, LayoutPolicy.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP * KeyboardHeightPolicy.KEYBOARD_ROW_COUNT)));

        LinearLayout modeColumn = LayoutPolicy.column(s);
        s.japaneseSymbolsKey = s.keyId(s.keyboardKey("123", "切换到数字和符号", () -> {
            s.keyboardLayer = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                ? KeyboardLayout.Layer.LETTERS : KeyboardLayout.Layer.SYMBOLS;
            s.imeLetterRows.rebuildKeyRows();
            s.render();
        }), "SoftLayer");
        // 左列：◀ 光标左移、→ 结束连点切换（或光标右移），再是 123、☺（工具栏没有表情时）、切换，各占一格。
        addJapaneseSideKey(modeColumn, s.iconKey(KeyboardIconKey.Kind.CURSOR_LEFT, "◀",
            "光标左移", this::moveJapaneseCaretLeft), 1);
        addJapaneseSideKey(modeColumn, s.iconKey(KeyboardIconKey.Kind.TOGGLE_NEXT, "→",
            "结束连点切换，开始下一个假名；没有组字时光标右移", this::advanceJapaneseToggle), 1);
        boolean swapLanguageNumber = s.localSettings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS);
        // 打开「中英键轮换其他语言」时这个键也按轮换走（日语之后是下一种其他语言或回到中文），键面写它要切到的语言；否则从日语只能到英文、英文再轮回日语，回不到中文。它和新设计底行一样放在空格与回车之间。
        Button language = s.keyId(s.keyboardKey(s.japaneseLanguageKeyLabel(), s.japaneseLanguageKeyDescription(),
            s::languageKeyTapped), "SoftLanguage");
        s.bindInputMethodPicker(language);
        addJapaneseSideKey(modeColumn, swapLanguageNumber ? language : s.japaneseSymbolsKey, 1);
        boolean emojiKey = s.japaneseSideEmojiKey();
        if (emojiKey) {
            addJapaneseSideKey(modeColumn, s.keyId(s.keyboardKey("☺", "打开表情浏览", s.imePanels::showEmojiPicker),
                "SoftEmoji"), 1);
        }
        if (s.offersGlobeKey()) {
            Button globe = s.keyId(s.keyboardKey("切换", "切换到下一个输入法",
                s::switchToNextInputMethodAfterCommit), "SoftGlobe");
            s.bindInputMethodPicker(globe);
            addJapaneseSideKey(modeColumn, globe, 1);
        }
        container.addView(modeColumn, LayoutPolicy.weightedMatchParentParams(0.17f));

        LinearLayout grid = LayoutPolicy.column(s);
        java.util.List<JapaneseNineKeyLayout.Key> keys = s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS
            ? JapaneseNineKeyLayout.digitKeys() : JapaneseNineKeyLayout.keys();
        for (int rowIndex = 0; rowIndex < 4; rowIndex++) {
            LinearLayout row = LayoutPolicy.row(s);
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
            grid.addView(row, LayoutPolicy.weightedWidthParams(1));
        }
        container.addView(grid, LayoutPolicy.weightedMatchParentParams(0.64f));

        LinearLayout side = LayoutPolicy.column(s);
        Runnable deleteAction = this::deleteJapaneseKana;
        Button delete = s.keyId(s.backspaceKey(deleteAction), "Backspace");
        s.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);
        addJapaneseSideKey(side, delete, 1);
        s.japaneseSpaceKey = s.keyId(s.keyboardKey("空白", "空白；左右滑动移动光标", s::space), "Space");
        s.imeBottomRow.bindSpaceCursor(s.japaneseSpaceKey);
        addJapaneseSideKey(side, s.japaneseSpaceKey, 1);
        addJapaneseSideKey(side, swapLanguageNumber ? s.japaneseSymbolsKey : language, 1);
        s.japaneseReturnKey = s.keyId(s.keyboardKey("改行", "改行", s::enter), "Enter");
        addJapaneseSideKey(side, s.japaneseReturnKey, 2);
        container.addView(side, LayoutPolicy.weightedMatchParentParams(0.19f));
        s.updateReturnKey();
    }

    void commitNineKeyLiteral(String text) {
        if (s.connection == null) return;
        if (s.dedicatedEnglish && text.length() == 1) {
            s.commitEnglishLiteral(text.charAt(0));
            return;
        }
        s.command(2);
        s.commitLiteralText(s.fullWidthOutput(text));
    }

    void commitNineKeyPeriod() {
        if (s.dedicatedEnglish || !s.punctuation('.')) commitNineKeyLiteral(".");
    }

    void chooseNineKeySpelling(long session, long generation, int index) {
        if (session == 0 || session != s.session || s.view == null
                || CandidateGlossPolicy.strictOr(s.view.opt("session"), Long.MIN_VALUE) != session
                || CandidateGlossPolicy.strictOr(s.view.opt("generation"), Long.MIN_VALUE) != generation)
            return;
        try { s.apply(NativeClient.chooseNineKeySpelling(s.session, generation, index)); }
        catch (JSONException | LinkageError error) { s.fail(error); }
    }

    /** 拼音栏里一项的高度：侧栏三行键高里放得下五六项，多的纵向滚动。 */
    static final int SPELLING_ROW_DP = 36;

    /** `nine_key_spellings` 里非空的各项及它们在数组里的下标（选择时交给引擎的是下标）。 */
    record Spellings(long generation, java.util.List<String> values, java.util.List<Integer> indices) {
        static Spellings of(JSONObject view) {
            JSONArray spellings = view == null ? null : view.optJSONArray("nine_key_spellings");
            long generation = view == null ? -1 : CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
            int count = spellings == null ? 0 : spellings.length();
            java.util.List<String> values = new java.util.ArrayList<>(count);
            java.util.List<Integer> indices = new java.util.ArrayList<>(count);
            for (int index = 0; index < count; index++) {
                String spelling = InputViewValuePolicy.text(spellings.opt(index));
                if (!spelling.isEmpty()) {
                    values.add(spelling);
                    indices.add(index);
                }
            }
            return new Spellings(generation, java.util.List.copyOf(values), java.util.List.copyOf(indices));
        }
    }

    /** 侧栏拼音列的一项：和标点列一样是侧栏底上不带键帽的字，宽度铺满侧栏。 */
    Button spellingColumnButton(LinearLayout column, Runnable action) {
        Button key = s.button(column, "", action);
        if (key instanceof KeyboardPressButton press) press.setKeyboardRole(KeyboardKeyRole.PLAIN);
        KeyboardTypographyPolicy.setKeyTextSize(key, 14);
        ViewPolicy.setSingleLine(key);
        // 最长的拼音（zhuang、shuang）在窄侧栏里等比缩小字号放下，而不是被侧栏边缘切掉（#5591）。
        ViewPolicy.setAutoSizeSp(key, 9, 14, 1);
        ViewPolicy.clearMinimumHeight(key);
        ViewPolicy.setHorizontalPadding(key, s.pixels(2));
        key.setLayoutParams(LayoutPolicy.matchWidthHeightPx(s.pixels(SPELLING_ROW_DP)));
        return key;
    }

    /** `suppressed`：显示诊断提示时不出选择条（与候选行一样让位给提示）。 */
    void renderNineKeySpellings(boolean suppressed) {
        if (s.nineKeySpellings == null || s.nineKeySpellingScroll == null) return;
        int layout = s.displayedTouchLayout(s.view);
        // 注音 9 键的选择条列出当前要钉住的那个音节的各个读音（ㄋㄧˇ、ㄌㄧˇ），拼音九键的列出拼音。
        boolean zhuyin = layout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT;
        boolean fourteenKey = layout == MSIMEInputService.FOURTEEN_KEY_LAYOUT;
        Spellings spellings = Spellings.of(s.view);
        boolean visible = !suppressed && s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && (MSIMEInputService.drawsKeyGrid(layout) || zhuyin)
            && !spellings.values().isEmpty();
        if (visible) ViewPolicy.show(s.nineKeySpellingScroll);
        else ViewPolicy.hide(s.nineKeySpellingScroll);
        // 拼音列盖住整条侧栏：标点列让出来（仍占位，侧栏宽度不变），没有拼音可选时再露出来。
        if (nineKeyPunctuation != null && nineKeyPunctuation.getParent() == s.nineKeySpellingScroll.getParent()) {
            if (visible) ViewPolicy.setInvisible(nineKeyPunctuation);
            else ViewPolicy.show(nineKeyPunctuation);
        }
        s.nineKeySpellingScroll.setContentDescription(zhuyin ? "注音读音选择"
            : fourteenKey ? "14 键拼音选择" : "九键拼音选择");
        if (!visible) {
            s.nineKeySpellingIndices = java.util.List.of();
            s.nineKeySpellingGeneration = -1;
            for (Button key : s.nineKeySpellingButtons) ViewPolicy.hide(key);
            return;
        }
        boolean column = s.nineKeySpellings.getOrientation() == LinearLayout.VERTICAL;
        // 换了一组拼音（新的一代）时回到第一项：第一项是引擎的首选。
        if (column && spellings.generation() != s.nineKeySpellingGeneration)
            s.nineKeySpellingScroll.scrollTo(0, 0);
        s.nineKeySpellingGeneration = spellings.generation();
        java.util.List<String> values = spellings.values();
        s.nineKeySpellingIndices = spellings.indices();
        while (s.nineKeySpellingButtons.size() < values.size()) {
            int slot = s.nineKeySpellingButtons.size();
            Runnable choose = () -> {
                if (slot < s.nineKeySpellingIndices.size()) {
                    chooseNineKeySpelling(s.session, s.nineKeySpellingGeneration,
                        s.nineKeySpellingIndices.get(slot));
                }
            };
            Button key;
            if (column) {
                key = spellingColumnButton(s.nineKeySpellings, choose);

            } else {
                key = s.button(s.nineKeySpellings, "", choose);
                LinearLayout.LayoutParams params = (LinearLayout.LayoutParams) key.getLayoutParams();
                params.width = LinearLayout.LayoutParams.WRAP_CONTENT;
                params.weight = 0;
                key.setLayoutParams(params);
            }
            key.setContentDescription("选择拼音");
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
                key.setContentDescription(NineKeyPanelPolicy.spellingDescription(spelling, zhuyin));
            }
        }
    }
}
