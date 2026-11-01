package app.msime.android.home;

import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.DimensionPolicy;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.NativeClient;
import app.msime.android.ReadAloudSpeaker;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.VocabularyReviewDocument;
import app.msime.android.VocabularyReviewModel;
import app.msime.android.VocabularyReviewText;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.time.LocalDate;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 背单词：词书、每日新词、导入与删除词表、复习卡片和清空进度，与 iOS 的 `VocabularyReviewSettingsView` 和共享设置页同样的内容和文字。
 *
 * <p>这里不做任何排期。间隔、难易系数、一次遗忘的代价和队列的顺序都归 `client-services::vocabulary`，经 `NativeClient.vocabularyReview`（C ABI `msime_client_vocabulary_review`）返回完整状态；页面只画 {@link VocabularyReviewModel}，报告按下了哪个按钮。
 *
 * <p>进度放在共享状态目录（与打字统计同一个：偏好目录，还没有配置时是 `bootstrap/state`），与 Tauri 合包的背单词页是同一份。内置词书由 Bootstrap 解到 `files/bootstrap/wordbooks`，所以资源根传 `files/bootstrap`：共享层在它下面的 `wordbooks/` 里找书，不向上找；这个目录不随词库在线更新变动。
 *
 * <p>请求带上插件目录（`plugins`，偏好目录下的 `plugins/`，键盘读的那一份）：单词本插件按 `pack-<插件 id>` 列在内置书之后、导入的书之前，在插件页卸载，这里不提供删除。插件详情的「去背单词」经 {@link #openBook} 打开这一页并选中那本书。
 *
 * <p>同一时间只有一个请求在途：连按两次「认识」时，第二次会按第一次已经替换掉的状态排期。
 */
public final class VocabularyReviewPage extends ReloadingDetailPage {
    /** 一次请求的结果：成功时 {@link #model} 非空，失败时 {@link #failure} 是要展示的一句话。 */
    private record Outcome(@Nullable VocabularyReviewModel model, String failure) {
        static Outcome failed(String failure) { return new Outcome(null, failure); }
    }

    /** 打开时要选中的书（插件详情的「去背单词」）。键名带连字符，外部深链的参数键（`HostDeepLink.isAllowedArgKey`）伪造不了它，所以别的应用不能借深链改掉用户选的书。 */
    private static final String ARG_SELECT_BOOK = "select-book";

    /** 打开背单词并选中 `book`（例如单词本插件的 `pack-<id>`）。 */
    static void openBook(Context context, String book) {
        Bundle args = new Bundle();
        args.putString(ARG_SELECT_BOOK, book);
        SettingsNavigator.open(context, PageId.VOCABULARY_REVIEW, args);
    }

    /** 导入只认文本：CSV、TSV 和 TXT。有的文件管理器把 .csv 报成 Excel 的类型，一并列上。 */
    private static final String[] IMPORT_TYPES = {"text/*", "application/csv", "application/vnd.ms-excel"};

    @Nullable private VocabularyReviewModel model;
    /** 卡片下面的「朗读」：第一次读时才连接系统语音引擎，页面销毁时断开。 */
    @Nullable private ReadAloudSpeaker speaker;
    /** 在途的请求数。读取、作答和设置只在没有请求在途时发出；导入不依赖当前状态，选完文件就发，不会因为恰好有一次读取在途而丢掉用户选的文件（共享入口的调用都在 {@link HostTask} 的同一个工作线程上排队）。 */
    private int inFlight;
    /** 当前卡片是否已翻开；换到下一张时复位，否则每一轮的第二张卡片会先露出释义。 */
    private boolean revealed;

    private final ActivityResultLauncher<String[]> openDocument =
        registerForActivityResult(new ActivityResultContracts.OpenDocument(), this::onImportPicked);

    @Override protected void onContentReady() {
        if (model != null) render();
    }

    @Override protected void reload() {
        // 有请求在途时不另起一次读取：它返回的完整状态就是最新的。
        if (inFlight > 0) return;
        act(operation("load"));
    }

    @Override protected void onContentDestroyed() {
        // 视图没了，在途请求的结果不会再送回来（HostTask 只在视图还在时交付）。
        inFlight = 0;
    }

    // ---- 数据 ----

    private void act(@Nullable JSONObject action) {
        if (action == null || inFlight > 0) return;
        inFlight++;
        HostTask.run(this, context -> call(context, action), this::deliver);
    }

    private void deliver(@Nullable Outcome outcome) {
        inFlight = Math.max(0, inFlight - 1);
        if (outcome == null) outcome = Outcome.failed(VocabularyReviewText.UNAVAILABLE);
        if (outcome.model() != null) {
            model = outcome.model();
        } else {
            MsToast.show(requireContext(), outcome.failure());
        }
        render();
        selectRequestedBook();
    }

    /** 第一次读到状态后选中参数里要的那本书，只做一次；书不在列表里（插件已卸载）时什么也不做。 */
    private void selectRequestedBook() {
        Bundle args = getArguments();
        String book = args == null ? null : args.getString(ARG_SELECT_BOOK);
        VocabularyReviewModel current = model;
        if (book == null || current == null || inFlight > 0) return;
        args.remove(ARG_SELECT_BOOK);
        if (book.equals(current.selected())) return;
        for (VocabularyReviewModel.Wordbook wordbook : current.wordbooks()) {
            if (wordbook.id().equals(book)) {
                setSettings(book, current.newPerDay());
                return;
            }
        }
    }

    @Nullable private static JSONObject operation(String name) {
        try {
            return new JSONObject().put("operation", name);
        } catch (JSONException error) {
            return null;
        }
    }

    private static Outcome call(Context context, JSONObject action) {
        String request = request(context, action);
        if (request == null) return Outcome.failed(VocabularyReviewText.UNAVAILABLE);
        return answer(NativeClient.vocabularyReview(request));
    }

    /** 共享入口的请求：状态目录、内置词书的资源根、插件目录、本机的今天和动作。 */
    @Nullable private static String request(Context context, JSONObject action) {
        File files = context.getFilesDir();
        String directory = HostStore.sharedStateDirectory(context);
        if (files == null || directory.isEmpty()) return null;
        String preferences = HostStore.directory(context);
        try {
            JSONObject request = new JSONObject()
                .put("directory", directory)
                .put("resources", new File(files, "bootstrap").getAbsolutePath());
            // 单词本插件在键盘读的插件目录里；还没完成首次设置时没有它，只列内置和导入的书。
            if (!preferences.isEmpty()) request.put("plugins", new File(preferences, "plugins").getAbsolutePath());
            return request
                // 本机的日历和时区只有这个进程知道，共享层从不自己推算哪天是今天。
                .put("day", LocalDate.now().toString())
                .put("action", action)
                .toString();
        } catch (JSONException error) {
            return null;
        }
    }

    private static Outcome answer(String response) {
        try {
            JSONObject root = new JSONObject(response);
            if (!JsonPolicy.strictTrue(root.opt("ok")))
                return Outcome.failed(VocabularyReviewText.failureMessage(JsonPolicy.strictStringOrEmpty(root.opt("error"))));
            VocabularyReviewModel parsed = VocabularyReviewDocument.from(root.optJSONObject("value"));
            return parsed == null ? Outcome.failed(VocabularyReviewText.UNAVAILABLE) : new Outcome(parsed, "");
        } catch (JSONException error) {
            return Outcome.failed(VocabularyReviewText.UNAVAILABLE);
        }
    }

    // ---- 渲染 ----

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        Context context = requireContext();
        target.removeAllViews();
        VocabularyReviewModel current = model;
        if (current == null) {
            GroupCard.add(target, null).note(inFlight > 0 ? "正在读取…" : VocabularyReviewText.UNAVAILABLE);
            return;
        }

        GroupCard today = GroupCard.add(target, "今日");
        today.addView(metrics(context, current));
        today.footer("复习只在这里进行，不会改变打字时的候选或组词。进度保存在本机，不会上传。");

        GroupCard books = GroupCard.add(target, "词书").withDividers(Ui.ROW_PADDING_H);
        VocabularyReviewModel.Wordbook selected = current.selectedWordbook();
        books.nav("词书", current.wordbooks().isEmpty() ? "还没有词书，可以先导入一个词表" : null,
            selected == null ? "未选择" : VocabularyReviewText.wordbookLabel(selected),
            current.wordbooks().isEmpty() ? null : this::chooseWordbook);
        books.slider("每日新词", 0, 200, 5, current.newPerDay(), String::valueOf, this::setNewPerDay);
        books.nav("导入词表（CSV / TXT）", null, null, this::pickImport);
        if (selected != null && selected.pack()) {
            books.note("这本书来自插件，在「插件」页卸载；卸载后复习进度仍会保留。");
        }
        if (selected != null && !selected.builtin() && !selected.pack()) {
            books.button("删除这个词表", "连同它的复习进度一起删除", "删除", () -> confirmRemove(selected));
        }
        books.footer("每行一个词，用逗号或制表符分隔：单词,音标,释义 或 单词,释义。释义里有逗号时用英文引号括起来。");

        GroupCard review = GroupCard.add(target, "复习");
        VocabularyReviewModel.Card card = current.current();
        if (current.needsWordbook()) {
            review.note("先选一本词书。");
        } else if (card == null) {
            review.note("今天的复习已经完成。");
        } else {
            review.addView(cardFace(context, card));
            // 词头是英文、本机又装着语音引擎时，卡片下面有「朗读」；它不翻开卡片，也不作答。
            String spoken = NativeClient.readAloudText(card.word(), "");
            if (spoken != null && ReadAloudSpeaker.engineInstalled(context))
                review.addView(readAloudButton(context, card.word(), spoken));
            review.addView(answerButtons(context));
        }
        review.footer("答「不认识」的词会在本次复习里再次出现；答「认识」的词按间隔安排到以后的某一天。");

        GroupCard reset = GroupCard.add(target, null);
        reset.button("清空复习进度", "已经学过的词会从头开始，词表本身会保留", "清空", this::confirmReset);
    }

    /** 今日待复习、已完成、未开始三个数，读屏作为一句读出。 */
    private static View metrics(Context context, VocabularyReviewModel current) {
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setSymmetricPaddingDp(context, row, Ui.ROW_PADDING_H, 14);
        row.addView(metric(context, "今日待复习", current.due()), LayoutPolicy.weightedWrapParams(1f));
        row.addView(metric(context, "已完成", current.answeredToday()), LayoutPolicy.weightedWrapParams(1f));
        row.addView(metric(context, "未开始", current.remaining()), LayoutPolicy.weightedWrapParams(1f));
        row.setContentDescription("今日待复习 " + current.due() + "，已完成 " + current.answeredToday()
            + "，未开始 " + current.remaining());
        row.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        return row;
    }

    private static View metric(Context context, String title, int value) {
        LinearLayout column = LayoutPolicy.column(context);
        ViewPolicy.setCenteredHorizontally(column);
        TextView number = ViewPolicy.centeredSingleLineLabel(context, String.valueOf(value), 28, 600,
            ThemeColorPolicy.accent(context));
        column.addView(number, LayoutPolicy.wrapParams());
        column.addView(ViewPolicy.centeredSingleLineLabel(context, title, 13, 400, ThemeColorPolicy.subText(context)),
            LayoutPolicy.wrapParams());
        column.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        return column;
    }

    /** 卡片正面：单词、音标（没有就不占行），点一下翻出释义。 */
    private View cardFace(Context context, VocabularyReviewModel.Card card) {
        LinearLayout face = LayoutPolicy.column(context);
        ViewPolicy.setCentered(face);
        face.setMinimumHeight(DimensionPolicy.pixels(context, 160));
        ViewPolicy.setSymmetricPaddingDp(context, face, Ui.ROW_PADDING_H, 20);
        face.addView(ViewPolicy.centeredLabel(context, card.word(), 32, 600, ThemeColorPolicy.text(context)),
            LayoutPolicy.wrapParams());
        if (!card.phonetic().isEmpty()) {
            LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
            params.topMargin = DimensionPolicy.pixels(context, 6);
            face.addView(ViewPolicy.centeredLabel(context, card.phonetic(), Ui.TEXT_ROW_SUBTITLE, 400,
                ThemeColorPolicy.subText(context)), params);
        }
        LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
        params.topMargin = DimensionPolicy.pixels(context, 10);
        face.addView(revealed
            ? ViewPolicy.centeredLabel(context, card.meaning(), Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context))
            : ViewPolicy.centeredLabel(context, "点击查看释义", 13, 400, ThemeColorPolicy.subText(context)), params);
        face.setContentDescription(revealed ? card.word() + "，" + card.meaning() : "显示 " + card.word() + " 的释义");
        if (!revealed) ViewPolicy.makeClickable(face, context, () -> {
            revealed = true;
            render();
        });
        return face;
    }

    private View readAloudButton(Context context, String word, String spoken) {
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setCenteredHorizontally(row);
        ViewPolicy.setPaddingDp(context, row, Ui.ROW_PADDING_H, 0, Ui.ROW_PADDING_H, 8);
        float radius = DimensionPolicy.pixels(context, Ui.GROUP_RADIUS);
        TextView button = ViewPolicy.textButton(context, "朗读", 15, 500, ThemeColorPolicy.accent(context),
            ViewPolicy.ripple(context, ThemeColorPolicy.accentSoft(context), radius), Ui.ACTION_BUTTON_MIN_HEIGHT,
            () -> {
                if (speaker == null) speaker = new ReadAloudSpeaker(context);
                speaker.speak(spoken, notice -> MsToast.show(requireContext(), notice));
            });
        button.setContentDescription("朗读 " + word);
        button.setAccessibilityDelegate(HomeButtonPolicy.buttonDelegate("朗读 " + word));
        row.addView(button, LayoutPolicy.wrapParams());
        return row;
    }

    @Override public void onDestroy() {
        if (speaker != null) speaker.shutdown();
        speaker = null;
        super.onDestroy();
    }

    private View answerButtons(Context context) {
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setPaddingDp(context, row, Ui.ROW_PADDING_H, 4, Ui.ROW_PADDING_H, 16);
        float radius = DimensionPolicy.pixels(context, Ui.GROUP_RADIUS);
        TextView unknown = ViewPolicy.textButton(context, "不认识", 16, 600, ThemeColorPolicy.accent(context),
            ViewPolicy.ripple(context, ThemeColorPolicy.accentSoft(context), radius), Ui.ACTION_BUTTON_MIN_HEIGHT,
            () -> answer(false));
        TextView known = ViewPolicy.textButton(context, "认识", 16, 600, ThemeColorPolicy.onAccent(context),
            ViewPolicy.ripple(context, ThemeColorPolicy.accent(context), radius), Ui.ACTION_BUTTON_MIN_HEIGHT,
            () -> answer(true));
        unknown.setAccessibilityDelegate(HomeButtonPolicy.buttonDelegate("不认识"));
        known.setAccessibilityDelegate(HomeButtonPolicy.buttonDelegate("认识"));
        row.addView(unknown, LayoutPolicy.weightedWrapParams(1f));
        LinearLayout.LayoutParams knownParams = LayoutPolicy.weightedWrapParams(1f);
        knownParams.setMarginStart(DimensionPolicy.pixels(context, 12));
        row.addView(known, knownParams);
        return row;
    }

    // ---- 操作 ----

    private void answer(boolean known) {
        VocabularyReviewModel current = model;
        VocabularyReviewModel.Card card = current == null ? null : current.current();
        if (card == null || inFlight > 0) return;
        revealed = false;
        try {
            act(new JSONObject().put("operation", "answer").put("wordbook", current.selected()).put("word", card.word()).put("known", known));
        } catch (JSONException error) {
            MsToast.show(requireContext(), VocabularyReviewText.UNAVAILABLE);
        }
    }

    private void chooseWordbook() {
        VocabularyReviewModel current = model;
        if (current == null) return;
        OptionSheet sheet = new OptionSheet(requireContext(), "词书", "内置词书、单词本插件和你导入的词表");
        sheet.option("未选择", current.selected().isEmpty(), () -> setSettings("", current.newPerDay()));
        for (VocabularyReviewModel.Wordbook book : current.wordbooks()) {
            sheet.option(VocabularyReviewText.wordbookLabel(book), book.id().equals(current.selected()),
                () -> setSettings(book.id(), current.newPerDay()));
        }
        sheet.show();
    }

    private void setNewPerDay(int value) {
        VocabularyReviewModel current = model;
        if (current == null || value == current.newPerDay()) return;
        setSettings(current.selected(), value);
    }

    private void setSettings(String wordbook, int newPerDay) {
        VocabularyReviewModel current = model;
        if (current == null) return;
        revealed = false;
        try {
            act(new JSONObject().put("operation", "set_settings").put("wordbook", wordbook)
                .put("new_per_day", newPerDay).put("session_limit", current.sessionLimit()));
        } catch (JSONException error) {
            MsToast.show(requireContext(), VocabularyReviewText.UNAVAILABLE);
        }
    }

    private void confirmRemove(VocabularyReviewModel.Wordbook book) {
        MessageDialog.confirm(requireContext(), "删除「" + book.name() + "」？",
            "这本词表和它的复习进度都会被删除，无法撤销。", "删除", () -> {
                try {
                    act(new JSONObject().put("operation", "remove").put("wordbook", book.id()));
                } catch (JSONException error) {
                    MsToast.show(requireContext(), VocabularyReviewText.UNAVAILABLE);
                }
            });
    }

    private void confirmReset() {
        MessageDialog.confirm(requireContext(), "清空全部复习进度？",
            "已经学过的词会从头开始，词表本身会保留。此操作无法撤销。", "清空", () -> {
                revealed = false;
                act(operation("reset"));
            });
    }

    private void pickImport() {
        openDocument.launch(IMPORT_TYPES);
    }

    private void onImportPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        inFlight++;
        revealed = false;
        HostTask.run(this, context -> importFrom(context, uri), this::deliver);
    }

    /** 读出选中的文件（不超过共享的导入上限），交给共享层解析；成功后共享层自动选中这本新词表。 */
    private static Outcome importFrom(Context context, Uri uri) {
        byte[] bytes;
        try (InputStream input = context.getContentResolver().openInputStream(uri)) {
            if (input == null) return Outcome.failed(VocabularyReviewText.UNREADABLE);
            bytes = HttpBodyPolicy.readBounded(input, VocabularyReviewText.MAX_IMPORT_BYTES);
        } catch (IOException | SecurityException error) {
            return Outcome.failed("读取文件失败，请重新选择。");
        }
        if (bytes == null) return Outcome.failed(VocabularyReviewText.UNREADABLE);
        String name = VocabularyReviewText.importName(LexiconPage.displayName(context, uri));
        final JSONObject action;
        try {
            action = new JSONObject().put("operation", "import").put("name", name)
                .put("text", VocabularyReviewText.decodeImport(bytes));
        } catch (JSONException error) {
            return Outcome.failed(VocabularyReviewText.UNREADABLE);
        }
        String request = request(context, action);
        if (request == null) return Outcome.failed(VocabularyReviewText.UNAVAILABLE);
        // 文本贴着上限时，加上目录和日期的整份请求会被入口按大小拒掉；那仍是「文件太大，不是可用的词表」，不是存储坏了。
        if (request.getBytes(StandardCharsets.UTF_8).length > VocabularyReviewText.MAX_IMPORT_BYTES)
            return Outcome.failed(VocabularyReviewText.UNREADABLE);
        return answer(NativeClient.vocabularyReview(request));
    }
}
