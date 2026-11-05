package app.msime.android;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import app.msime.android.core.InputViewValuePolicy;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 长按候选打开的「释义」卡片（#10915），内容规则在 {@link CandidateGlossCardModel}。卡片盖住整块按键区（展开的候选面板也在按键区里），候选栏和读音行留在上面；点候选栏、点卡片的「收起」、按返回键或收起键盘时关闭，组字结束或变了也关闭。原来的长按菜单（可上屏释义、朗读、加入生词本、词库操作）全部并进卡片：义项点了就上屏，其余在底部按钮上方的一行次要操作里。
 *
 * <p>画在键盘里，不弹 `PopupMenu`、`PopupWindow` 或对话框：弹出窗口会拿走编辑器的窗口焦点，Via 这类 WebView 浏览器随即收起键盘（#5653），删除词条的确认也就地换成一个按钮，不另开子菜单。
 *
 * <p>译文只走现有通路：离线释义在自己的线程上查这一个候选（整页释义的 worker 只排一个任务，新的挤掉旧的，卡片的查询放在那里可能被下一次刷新挤掉）；用户开着在线翻译服务、而这个候选在某个目标语言上离线和缓存都没有时，经同一个 {@link CandidateTranslationStore} 为它单独请求一次，回答进整页共用的缓存。打开卡片时记下会话和一个代次，关掉或重开就换代，迟到的回答对不上就丢掉。
 */
final class ImeGlossCard {
    /** 卡片对应的候选。{@code text} 是候选栏上显示的写法，{@code raw} 是 Engine 的原文（释义和译文按它查），{@code id} 是长按那一刻的候选身份，{@code displayed} 是候选栏上显示着的译文。 */
    record Target(String text, String raw, int source, JSONObject candidate, JSONObject id, String displayed) {}

    private final MSIMEInputService s;
    private final ExecutorService worker = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-gloss-card"));
    private LinearLayout root;
    private View shield;
    private Target target;
    private long session;
    private String editingText = "";
    private long epoch;
    private List<String> targets = List.of();
    private Map<String, String> offline = Map.of();
    private final Map<String, String> online = new HashMap<>();
    private CandidateGlossCardModel.Source onlineSource;
    private boolean offlinePending;
    private boolean onlinePending;
    /** 在线缓存可以读了：用户服务一打开就是，账号要等绑定核对完。 */
    private boolean onlineReady;
    private boolean confirmingRemoval;
    private boolean management;
    /** 当前方案允许插入译文（{@link MSIMEInputService#candidateGlossInsertionEnabled}），与原菜单列出可上屏释义的条件相同。 */
    private boolean insertable;

    ImeGlossCard(MSIMEInputService s) {
        this.s = s;
    }

    /** 建卡片和挡住候选栏的那一层，都先隐藏；由服务加进键盘面板，位置由 {@link #alignBelow} 跟着顶部一行走。 */
    void build(FrameLayout surface) {
        root = LayoutPolicy.column(s);
        root.setContentDescription("释义卡片");
        root.setAccessibilityPaneTitle(CandidateGlossCardModel.TITLE);
        int padding = s.pixels(10);
        ViewPolicy.setPadding(root, padding, s.pixels(6), padding, s.pixels(8));
        // 空白处的触摸不能穿到下面的键上。
        ViewPolicy.setClickable(root, true);
        ViewPolicy.hide(root);
        shield = new View(s);
        shield.setContentDescription(CandidateGlossCardModel.SHIELD_DESCRIPTION);
        ViewPolicy.bindClick(shield, this::close);
        ViewPolicy.hide(shield);
        surface.addView(root, LayoutPolicy.frameMatchParentParams());
        surface.addView(shield, LayoutPolicy.frameParamsPx(FrameLayout.LayoutParams.MATCH_PARENT, 0));
    }

    View root() { return root; }

    boolean visible() { return root != null && ViewPolicy.isVisible(root); }

    /** 卡片顶边对齐顶部一行的底边；挡层正好盖住顶部一行（候选栏和读音行），点上去只关卡片，不选词。 */
    void alignBelow(int top) {
        if (root != null && root.getLayoutParams() instanceof FrameLayout.LayoutParams params
                && params.topMargin != top) {
            params.topMargin = top;
            root.post(() -> root.setLayoutParams(params));
        }
        if (shield != null && shield.getLayoutParams() instanceof FrameLayout.LayoutParams params
                && params.height != top) {
            params.height = top;
            shield.post(() -> shield.setLayoutParams(params));
        }
    }

    /** 打开卡片；键盘还没建好或没有会话时返回 false，长按照旧落空。 */
    boolean open(Target next) {
        if (root == null || s.session == 0 || s.view == null || next == null || next.id() == null) return false;
        close();
        target = next;
        session = s.session;
        editingText = InputViewValuePolicy.editingText(s.view);
        long opened = ++epoch;
        management = s.candidateManagementEnabled();
        insertable = s.candidateGlossInsertionEnabled();
        targets = s.candidateTranslationTargets;
        offline = Map.of();
        online.clear();
        onlineSource = onlineAllowed() ? CandidateGlossCardModel.Source.online(s.candidateTranslationRoute.provider()) : null;
        onlineReady = false;
        readCachedOnline(opened);
        lookupOffline(opened);
        render();
        ViewPolicy.show(root);
        ViewPolicy.show(shield);
        coverKeys(true);
        s.imeStyler.applySkin();
        return true;
    }

    /** 关卡片，不重画；之后才到的离线或在线回答都对不上代次。 */
    void close() {
        epoch++;
        target = null;
        confirmingRemoval = false;
        offlinePending = false;
        onlinePending = false;
        onlineReady = false;
        if (root != null) {
            ViewPolicy.hide(root);
            root.removeAllViews();
        }
        if (shield != null) ViewPolicy.hide(shield);
        coverKeys(false);
    }

    /** 卡片开着时，被它盖住的按键和展开面板不进读屏，免得读屏用户摸到看不见的键。 */
    private void coverKeys(boolean covered) {
        int mode = covered ? View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            : View.IMPORTANT_FOR_ACCESSIBILITY_AUTO;
        for (View view : new View[] {s.keyRows, s.actionRow, s.expandedCandidateScroll,
                s.imeNineKeyPanel == null ? null : s.imeNineKeyPanel.root()}) {
            if (view != null && view.getImportantForAccessibility() != mode) view.setImportantForAccessibility(mode);
        }
    }

    /**
     * 每次 render 调用：会话换了（换了输入框）、组字结束或组字变了（实体键盘还能打字），卡片就不再属于眼前的候选，关掉。
     *
     * <p>候选只是换了一代（停顿后的整句重排、云候选和 AI 候选到达）而组字没变时卡片留着：停顿重排正好在用户停下来、伸手长按的那一刻到达，按换代关卡片会让卡片刚打开就消失。卡片上的内容属于候选文字，换代不影响它；要用候选身份的词库操作在执行时按文字在新一代里重新找（{@link #currentId}）。
     */
    void sync() {
        if (!visible()) return;
        if (s.session != session || s.view == null || !s.hasEngineComposition()
                || !editingText.equals(InputViewValuePolicy.editingText(s.view))) close();
    }

    void shutdown() {
        worker.shutdownNow();
    }

    // ---- 译文 ----

    private int scheme() {
        return s.view == null ? -1 : InputViewValuePolicy.scheme(s.view, -1);
    }

    private boolean plainComposition() {
        return s.view != null && "none".equals(InputViewValuePolicy.textOr(s.view, "local_mode", "none"))
            && scheme() != 3;
    }

    /** 在线译文的门槛与整页的账号路径、用户服务路径相同：没有本地模式、不是日文方案、方案显示释义、来源可以送翻译服务。 */
    private boolean onlineAllowed() {
        return target != null && plainComposition() && MSIMEInputService.schemeShowsGlosses(scheme())
            && CandidateTranslationPolicy.userTranslationSourceAllowed(target.source())
            && store() != null;
    }

    private CandidateTranslationStore store() {
        CandidateTranslationRoute.Provider provider = s.candidateTranslationRoute.provider();
        if (provider == CandidateTranslationRoute.Provider.ACCOUNT) return s.candidateTranslationStore;
        return s.candidateTranslationRoute.userService() ? s.userTranslationStore : null;
    }

    /**
     * 用户服务的缓存直接读。账号的缓存属于某个账号：在工作线程上取当前绑定，回到主线程再取一次，两次一致、而且 store 正绑在这个账号上（整页的账号路径每一代都会核对并绑定）才读缓存、才单独请求；不一致或还没绑上时卡片不用账号译文，也不替整页改绑定。
     */
    private void readCachedOnline(long opened) {
        if (onlineSource == null) return;
        if (onlineSource != CandidateGlossCardModel.Source.ACCOUNT) {
            copyCached(store());
            onlineReady = true;
            return;
        }
        onlinePending = true;
        try {
            worker.execute(() -> {
                SyncSwitch.Binding binding = SyncSignals.binding(s);
                s.main.post(() -> {
                    if (opened != epoch) return;
                    onlinePending = false;
                    String key = MSIMEInputService.candidateTranslationBindingKey(binding);
                    CandidateTranslationStore store = store();
                    if (store == null || !CandidateTranslationPolicy.acceptsBinding(key,
                            MSIMEInputService.candidateTranslationBindingKey(SyncSignals.binding(s)))
                            || !store.boundTo(key)) {
                        onlineSource = null;
                    } else {
                        copyCached(store);
                        onlineReady = true;
                        if (!offlinePending) requestMissing(opened);
                    }
                    render();
                });
            });
        } catch (RuntimeException rejected) {
            onlinePending = false;
            onlineSource = null;
        }
    }

    private void copyCached(CandidateTranslationStore store) {
        if (store == null || target == null) return;
        for (String language : targets) {
            String value = store.gloss(target.raw(), language);
            if (value != null) online.put(language, value);
        }
    }

    /** 离线释义：开着「候选英文释义」、方案显示释义时，查随包的英文释义，再查装了词典的其他目标语言，与整页的 scheduleCandidateGlosses 同一条通路。 */
    private void lookupOffline(long opened) {
        String resources = s.candidateGlossResources;
        if (!s.candidateEnglishGloss || resources.isEmpty() || target == null
                || !MSIMEInputService.schemeShowsGlosses(scheme())) {
            requestMissing(opened);
            return;
        }
        String stateRoot = s.candidateGlossStateRoot.isEmpty() ? null : s.candidateGlossStateRoot;
        List<String> languages = plainComposition()
            ? CandidateTranslationPolicy.offlineTargets(targets, resources, stateRoot) : List.of();
        String raw = target.raw();
        int source = Math.max(0, Math.min(255, target.source()));
        offlinePending = true;
        try {
            worker.execute(() -> {
                HashMap<String, String> found = new HashMap<>(languages.size() + 1);
                try {
                    JSONArray candidates = new JSONArray().put(new JSONObject().put("text", raw).put("source", source));
                    putGloss(found, "en", NativeClient.candidateGlosses(
                        CandidateGlossModel.request(0, candidates), resources), raw);
                    for (String language : languages) {
                        putGloss(found, language, NativeClient.candidateGlosses(
                            CandidateGlossModel.request(0, candidates, language), resources, stateRoot), raw);
                    }
                } catch (JSONException | RuntimeException | LinkageError ignored) {
                    // 查不到就当没有离线释义；不记录候选文字。
                }
                s.main.post(() -> {
                    if (opened != epoch) return;
                    offlinePending = false;
                    offline = Map.copyOf(found);
                    requestMissing(opened);
                    render();
                });
            });
        } catch (RuntimeException rejected) {
            offlinePending = false;
            requestMissing(opened);
        }
    }

    private static void putGloss(Map<String, String> found, String language, String response, String raw) {
        try {
            JSONArray entries = new JSONArray(CandidateGlossModel.decode(response).translations());
            for (int index = 0; index < entries.length(); index++) {
                JSONObject entry = entries.getJSONObject(index);
                if (raw.equals(entry.optString("text"))) found.put(language, entry.getString("translation"));
            }
        } catch (JSONException | RuntimeException error) {
            // 这一种语言查不到，不影响别的语言。
        }
    }

    /** 离线和缓存都没有的目标语言，为这一个候选请求一次。 */
    private void requestMissing(long opened) {
        if (opened != epoch || onlineSource == null || !onlineReady || offlinePending || target == null) return;
        List<String> missing = CandidateGlossCardModel.missing(targets, offline, online);
        CandidateTranslationStore store = store();
        if (missing.isEmpty() || store == null) return;
        String raw = target.raw();
        onlinePending = store.request(raw, missing, worker, arrived -> {
            if (opened != epoch) return;
            onlinePending = false;
            copyCached(store);
            render();
        });
    }

    // ---- 界面 ----

    private void render() {
        if (root == null || target == null) return;
        root.removeAllViews();
        CandidateGlossCardModel model = CandidateGlossCardModel.of(target.text(), targets, offline, online, onlineSource);
        LinearLayout header = LayoutPolicy.row(s);
        header.setGravity(Gravity.CENTER_VERTICAL);
        TextView title = ViewPolicy.textLabel(s, CandidateGlossCardModel.TITLE, 13);
        KeyboardTypographyPolicy.setKeyTextSize(title, 13);
        title.setAlpha(0.7f);
        title.setAccessibilityHeading(true);
        header.addView(title, LayoutPolicy.weightedWrapParams(1));
        Button close = actionButton(CandidateGlossCardModel.CLOSE, KeyboardKeyRole.GLYPH, this::close);
        close.setContentDescription(CandidateGlossCardModel.CLOSE_DESCRIPTION);
        header.addView(close, LayoutPolicy.wrapParams());
        root.addView(header, LayoutPolicy.matchWidthWrapParams());
        TextView candidate = ViewPolicy.textLabel(s, model.candidate(), 26);
        KeyboardTypographyPolicy.setKeyTextSize(candidate, 26);
        candidate.setContentDescription("候选：" + model.candidate());
        root.addView(candidate, LayoutPolicy.matchWidthWrapParams());

        LinearLayout body = LayoutPolicy.column(s);
        if (model.empty()) {
            TextView empty = ViewPolicy.textLabel(s, CandidateGlossCardModel.EMPTY, 15);
            KeyboardTypographyPolicy.setKeyTextSize(empty, 15);
            body.addView(empty, LayoutPolicy.matchWidthWrapParams(s, 6));
            String hint = CandidateGlossCardModel.emptyHint(
                s.candidateTranslationRoute.provider() != CandidateTranslationRoute.Provider.NONE,
                offlinePending || onlinePending);
            if (!hint.isEmpty()) {
                TextView note = ViewPolicy.textLabel(s, hint, 12);
                KeyboardTypographyPolicy.setKeyTextSize(note, 12);
                note.setAlpha(0.7f);
                body.addView(note, LayoutPolicy.matchWidthWrapParams(s, 2));
            }
        }
        for (CandidateGlossCardModel.Section section : model.sections()) {
            TextView label = ViewPolicy.textLabel(s, section.label(), 12);
            KeyboardTypographyPolicy.setKeyTextSize(label, 12);
            label.setAlpha(0.7f);
            body.addView(label, LayoutPolicy.matchWidthWrapParams(s, 6));
            WrapRowLayout senses = new WrapRowLayout(s, s.pixels(6), s.pixels(6));
            for (String sense : section.senses()) {
                Button button = actionButton(sense, KeyboardKeyRole.KEY, () -> insert(sense));
                button.setContentDescription(insertable ? section.label() + "：" + sense + "；点按上屏"
                    : section.label() + "：" + sense);
                ViewPolicy.setEnabledWithAlpha(button, insertable, 0.4f);
                senses.addView(button);
            }
            body.addView(senses, LayoutPolicy.matchWidthWrapParams(s, 4));
        }
        ScrollView scroll = new ScrollView(s);
        scroll.setVerticalScrollBarEnabled(false);
        scroll.addView(body, LayoutPolicy.scrollMatchWidthWrapParams());
        root.addView(scroll, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));

        LinearLayout secondary = secondaryActions(model);
        if (secondary.getChildCount() > 0) {
            HorizontalScrollView strip = new HorizontalScrollView(s);
            strip.setHorizontalScrollBarEnabled(false);
            strip.setContentDescription("更多操作");
            strip.addView(secondary);
            root.addView(strip, LayoutPolicy.matchWidthWrapParams(s, 6));
        }

        LinearLayout footer = LayoutPolicy.row(s);
        Button commit = actionButton(CandidateGlossCardModel.COMMIT, KeyboardKeyRole.RETURN,
            () -> insert(model.primary()));
        Button copy = actionButton(CandidateGlossCardModel.COPY, KeyboardKeyRole.KEY, () -> copy(model.primary()));
        boolean available = !model.primary().isEmpty();
        // 不可用的按钮画淡：按钮底色由皮肤决定，只看底色分不出能不能点。
        ViewPolicy.setEnabledWithAlpha(commit, insertable && available, 0.4f);
        ViewPolicy.setEnabledWithAlpha(copy, available, 0.4f);
        commit.setContentDescription(CandidateGlossCardModel.COMMIT + model.unavailableReason());
        copy.setContentDescription(CandidateGlossCardModel.COPY + model.unavailableReason());
        LinearLayout.LayoutParams left = LayoutPolicy.weightedWrapParams(1);
        left.rightMargin = s.pixels(4);
        LinearLayout.LayoutParams right = LayoutPolicy.weightedWrapParams(1);
        right.leftMargin = s.pixels(4);
        footer.addView(commit, left);
        footer.addView(copy, right);
        root.addView(footer, LayoutPolicy.matchWidthWrapParams(s, 6));
        if (ViewPolicy.isVisible(root)) s.imeStyler.applySkinToView(root);
    }

    /** 原长按菜单里的朗读、加入生词本和词库操作，顺序与原菜单相同：词库操作在前，朗读、加入生词本在后。用功能键的底色，和上面可点上屏的义项分开；删除的确认用强调色。 */
    private LinearLayout secondaryActions(CandidateGlossCardModel model) {
        LinearLayout row = LayoutPolicy.row(s);
        String translation = model.empty() ? target.displayed() : model.translation();
        if (management) {
            for (CandidateManagementAction action : CandidateManagementAction.values()) {
                if (action.confirmationRequired() && confirmingRemoval) {
                    addSecondary(row, action.confirmationTitle(), KeyboardKeyRole.RETURN, () -> manage(action));
                } else if (action.confirmationRequired()) {
                    addSecondary(row, action.title(), KeyboardKeyRole.ACCENT, () -> {
                        confirmingRemoval = true;
                        render();
                    });
                } else {
                    addSecondary(row, action.title(), KeyboardKeyRole.ACCENT, () -> manage(action));
                }
            }
        }
        String readAloud = s.readAloudText(target.text(), translation);
        if (readAloud != null)
            addSecondary(row, CandidateManagementAction.READ_ALOUD_TITLE, KeyboardKeyRole.ACCENT, () -> s.readAloud(readAloud));
        if (CandidateManagementAction.offersPersonalWordbook(CandidateTranslationPolicy.insertionGlosses(translation)))
            addSecondary(row, CandidateManagementAction.PERSONAL_WORDBOOK_TITLE, KeyboardKeyRole.ACCENT,
                () -> s.addCandidateToPersonalWordbook(target.text(), translation,
                    target.candidate() == null ? "" : s.candidateDisplayedReading(target.candidate())));
        return row;
    }

    private void addSecondary(LinearLayout row, String label, KeyboardKeyRole role, Runnable action) {
        Button button = actionButton(label, role, action);
        LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
        params.rightMargin = s.pixels(6);
        row.addView(button, params);
    }

    private Button actionButton(String label, KeyboardKeyRole role, Runnable action) {
        KeyboardPressButton button = ViewPolicy.newPressButton(s);
        button.setKeyboardRole(role);
        ViewPolicy.setTextSizeLabel(button, label, 14);
        KeyboardTypographyPolicy.setKeyTextSize(button, 14);
        button.setContentDescription(label);
        ViewPolicy.clearMinimumWidth(button);
        ViewPolicy.setMinimumHeight(button, s.pixels(36));
        LayoutPolicy.setSymmetricPaddingDp(button, s, 12, 4);
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
        return button;
    }

    // ---- 操作 ----

    /** 上屏一个译文并结束组字（与 Ctrl+Enter 义项页同一条 {@link MSIMEInputService#commitTranslationSense}）。卡片开着就说明组字没变（{@link #sync}），不用再核对候选身份。 */
    private void insert(String translation) {
        if (target == null || translation == null || translation.isEmpty()) return;
        close();
        s.commitTranslationSense(translation);
    }

    private void copy(String translation) {
        if (translation == null || translation.isEmpty()) return;
        ClipboardManager manager = s.getSystemService(ClipboardManager.class);
        if (manager == null) {
            s.notice(CandidateGlossCardModel.COPY_FAILED);
            return;
        }
        try {
            manager.setPrimaryClip(ClipData.newPlainText(CandidateGlossCardModel.TITLE, translation));
            s.notice(CandidateGlossCardModel.COPIED);
        } catch (RuntimeException error) {
            s.notice(CandidateGlossCardModel.COPY_FAILED);
        }
    }

    private void manage(CandidateManagementAction action) {
        Target current = target;
        if (current == null) return;
        close();
        JSONObject id = currentId(current);
        if (id == null) {
            s.notice(CandidateGlossCardModel.STALE);
            return;
        }
        s.editCandidate(id, action);
    }

    /** 候选在当前这一代里的身份：长按那一刻的身份还是当前的就用它；换了一代就在当前候选里按原文找同一个词；找不到为 null。 */
    private JSONObject currentId(Target current) {
        if (s.candidateVersionIsCurrent(current.id())) return current.id();
        JSONArray candidates = s.view == null ? null : s.view.optJSONArray("candidates");
        for (int index = 0; candidates != null && index < candidates.length(); index++) {
            JSONObject candidate = candidates.optJSONObject(index);
            JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
            if (id != null && current.raw().equals(InputViewValuePolicy.textOr(candidate, "text", ""))
                    && s.candidateVersionIsCurrent(id)) return id;
        }
        return null;
    }
}
