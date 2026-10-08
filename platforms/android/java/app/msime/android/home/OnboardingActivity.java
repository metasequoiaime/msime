package app.msime.android.home;

import app.msime.android.KeyboardGeometry;
import android.content.Context;
import android.graphics.drawable.GradientDrawable;
import android.os.Bundle;
import android.view.GestureDetector;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.animation.AnimationUtils;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.activity.OnBackPressedCallback;
import androidx.annotation.DrawableRes;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.widget.NestedScrollView;
import app.msime.android.AppEdition;
import app.msime.android.BoundsPolicy;
import app.msime.android.FirstRunPreparation;
import app.msime.android.KeyboardScheme;
import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.R;
import app.msime.android.SchemePreferences;
import app.msime.android.ViewPolicy;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.progressindicator.LinearProgressIndicator;
import org.json.JSONObject;

/**
 * 新手引导：四步，从「把键盘加进系统」走到「登录能多得到什么」。
 *
 * <p>The design's Android flow: a progress bar rather than a counter, an accent glyph, a kicker, a large regular-weight title and a body line, then the step's own content. The left button reads 跳过 on the first step and 上一步 after it; a horizontal swipe moves between steps too.
 *
 * <p>这几页上的每个控件都改真实的设置。方案卡片经 {@link SchemePreferences#withScheme} 写入，与设置页的方案选择是同一套映射；「显示译文」开关就是键盘读的离线英文释义偏好。首次安装时偏好要等词库准备完成才读得到，点下的选择先由 {@link OnboardingChoices} 记下并立即显示，偏好可读后自动写入，用户不必等准备完成，离开引导也不会丢。The last step's 登录 opens {@link LoginSheet} (Apple / Google / 邮箱，按后端接受的方式), shown only when sign-in is offered; 稍后再说 sits beside it and leaves the flow without signing in, as on iOS and in the design's Android footer (`ob.andLeft`): 跳过 shows only on the first step, so without it the last step had no way out but opening the sheet and closing it again. After a successful sign-in, after the sheet was closed without signing in, or when sign-in is not offered at all, the main button reads 开始使用 and 稍后再说 goes away.
 */
public final class OnboardingActivity extends AppCompatActivity {
    /** 引导自己的 SharedPreferences：「已看过」和 {@link OnboardingChoices} 记下的待保存选择都在这里。 */
    static final String STORE = "msime_onboarding_v1";
    private static final String SEEN = "seen";
    private static final int STEP_ENABLE = 0;
    private static final int STEP_SCHEMES = 1;
    private static final int STEP_TRANSLATION = 2;
    private static final int STEP_SYNC = 3;
    private static final String STATE_PAGE = "onboarding-page";

    /** Whether the flow has run on this device. */
    public static boolean seen(Context context) {
        return context.getSharedPreferences(STORE, MODE_PRIVATE).getBoolean(SEEN, false);
    }

    private static void markSeen(Context context) {
        context.getSharedPreferences(STORE, MODE_PRIVATE).edit().putBoolean(SEEN, true).apply();
    }

    /** One scheme card: what it is called here, its supporting line, and the scheme it selects. */
    private record SchemeCard(String label, String detail, KeyboardScheme scheme) {}

    /** 本版本走的步骤，顺序固定。只有一个方案的版本（五笔版）没有方案可选，跳过「选一套输入方案」这一步。 */
    private final int[] steps = AppEdition.current().offersSchemeChoice()
        ? new int[] {STEP_ENABLE, STEP_SCHEMES, STEP_TRANSLATION, STEP_SYNC}
        : new int[] {STEP_ENABLE, STEP_TRANSLATION, STEP_SYNC};
    private final int pages = steps.length;

    private int page;
    @Nullable private JSONObject snapshot;
    private boolean loaded;
    /** 意外异常后自动重试保存的次数与间隔，见 {@link #syncChoices}。 */
    private static final int SYNC_RETRY_LIMIT = 3;
    private static final long SYNC_RETRY_DELAY_MS = 1500;
    private final android.os.Handler retryHandler = new android.os.Handler(android.os.Looper.getMainLooper());
    private int syncRetries;
    /** What the last write on this page did, shown under the page's controls until the page changes. */
    @Nullable private String note;
    /** Whether the last step can offer sign-in; null until the off-thread check answers, which reads as not offered. */
    @Nullable private SignIn.State account;
    private boolean signingIn;
    /** 在最后一步打开过登录面板又没登录就关掉了：主按钮改为「开始使用」，不再反复弹出面板，「稍后再说」随之收起。 */
    private boolean declined;
    private OnBackPressedCallback back;
    private GestureDetector swipe;
    /** 首次安装时，引导页打开时词库还在准备，准备完成前读不到偏好设置。准备完成时重新读取并写入待保存的选择；进入失败或重新开始准备时，方案页和译文页的脚注跟着换。 */
    private final FirstRunPreparation.Listener preparation = status -> {
        if (isFinishing() || isDestroyed()) return;
        if (status == FirstRunPreparation.State.READY) syncChoices();
        else if (steps[page] == STEP_SCHEMES || steps[page] == STEP_TRANSLATION) render(false);
    };

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        setContentView(R.layout.activity_onboarding);
        // The layout's max is the four-step flow; an edition with a single scheme skips the scheme step, and the bar must still fill on its last page.
        LinearProgressIndicator progress = findViewById(R.id.onboarding_progress);
        progress.setMax(pages);
        progress.setIndicatorColor(Ui.accent(this));
        progress.setTrackColor(Ui.accentSoft(this));
        if (state != null) page = KeyboardGeometry.bounded(state.getInt(STATE_PAGE, 0), 0, pages - 1);
        ViewPolicy.bindClick(findViewById(R.id.onboarding_skip), this::finishFlow);
        ViewPolicy.bindClick(findViewById(R.id.onboarding_previous), () -> go(page - 1));
        ViewPolicy.bindClick(findViewById(R.id.onboarding_later), this::finishFlow);
        ViewPolicy.bindClick(findViewById(R.id.onboarding_next), () -> {
            if (page < pages - 1) go(page + 1);
            else if (account == SignIn.State.OFFERED && !declined) signIn();
            else finishFlow();
        });
        // Back steps back through the flow; on the first step it leaves without marking the flow seen, as it always has, so the guide comes back on the next launch.
        back = new OnBackPressedCallback(false) {
            @Override public void handleOnBackPressed() { go(page - 1); }
        };
        getOnBackPressedDispatcher().addCallback(this, back);
        swipe = new GestureDetector(this, new GestureDetector.SimpleOnGestureListener() {
            @Override public boolean onFling(@Nullable MotionEvent start, @NonNull MotionEvent end,
                    float velocityX, float velocityY) {
                if (start == null) return false;
                float dx = end.getX() - start.getX();
                float dy = end.getY() - start.getY();
                // The design's rule: at least 50 px across, and clearly more across than down, so a vertical scroll never turns the page.
                if (Math.abs(dx) < Ui.dp(OnboardingActivity.this, 50) || Math.abs(dx) < Math.abs(dy) * 1.5f) return false;
                if (dx < 0 && page < pages - 1) go(page + 1);
                else if (dx > 0 && page > 0) go(page - 1);
                return true;
            }
        });
        render(false);
        // 进程在准备途中被杀（引导第一步正把用户送去系统设置，小米等系统会杀掉退到后台的应用）后，系统只重建栈顶的引导页，不重建下面负责启动准备的 HomeActivity；不在这里启动，准备就一直停在 IDLE，方案页永远读不到偏好。startIfNeeded 本身幂等。
        OnboardingChoices.watch(this);
        FirstRunPreparation.startIfNeeded(this);
        syncChoices();
        FirstRunPreparation.observe(preparation);
        probeAccount();
    }

    @Override protected void onDestroy() {
        FirstRunPreparation.stopObserving(preparation);
        retryHandler.removeCallbacksAndMessages(null);
        super.onDestroy();
    }

    @Override protected void onSaveInstanceState(@NonNull Bundle state) {
        super.onSaveInstanceState(state);
        state.putInt(STATE_PAGE, page);
    }

    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        swipe.onTouchEvent(event);
        return super.dispatchTouchEvent(event);
    }

    /** The first step reports system state; coming back from system settings or the keyboard picker returns focus here, which is when it may have changed. */
    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus && page == 0) render(false);
    }

    private void finishFlow() {
        markSeen(this);
        finish();
    }

    private void go(int target) {
        if (target < 0 || target >= pages || target == page) return;
        page = target;
        note = null;
        render(true);
    }

    private void render(boolean animate) {
        ((LinearProgressIndicator) findViewById(R.id.onboarding_progress))
            .setProgressCompat(page + 1, animate);
        ViewPolicy.setVisible(findViewById(R.id.onboarding_skip), page == 0);
        ViewPolicy.setVisible(findViewById(R.id.onboarding_previous), page != 0);
        boolean offer = page == pages - 1 && account == SignIn.State.OFFERED && !declined;
        MaterialButton next = findViewById(R.id.onboarding_next);
        next.setText(page < pages - 1 ? R.string.onboarding_next
            : offer ? R.string.onboarding_sign_in : R.string.onboarding_done);
        ViewPolicy.setEnabled(next, !signingIn);
        View later = findViewById(R.id.onboarding_later);
        ViewPolicy.setVisible(later, offer);
        ViewPolicy.setEnabled(later, !signingIn);
        back.setEnabled(page > 0);

        LinearLayout column = findViewById(R.id.onboarding_page);
        column.removeAllViews();
        switch (steps[page]) {
            case STEP_ENABLE -> enable(column);
            case STEP_SCHEMES -> schemes(column);
            case STEP_TRANSLATION -> translation(column);
            default -> sync(column);
        }
        if (animate) {
            ((NestedScrollView) findViewById(R.id.onboarding_scroll)).scrollTo(0, 0);
            column.startAnimation(AnimationUtils.loadAnimation(this, R.anim.fade_through_in));
        }
    }

    // ---- steps ----

    /** 当前这一步是第几步：跳过了选方案的版本，后面的步骤依次往前挪。 */
    private String ordinal() {
        return "第" + "一二三四".charAt(page) + "步";
    }

    private void enable(LinearLayout column) {
        header(column, R.drawable.ic_ms_keyboard, ordinal() + " · 约 30 秒", "把水杉加进键盘",
            "在系统设置里启用水杉，并设为默认输入法，之后在任何应用里都能直接用。");
        boolean enabled = ImeSetup.enabled(this);
        boolean current = enabled && ImeSetup.isDefault(this);
        LinearLayout card = card(column, 6);
        check(card, getString(R.string.settings_check_enabled), enabled,
            getString(R.string.settings_action_enable), () -> ImeSetup.openSettings(this), false);
        check(card, getString(R.string.settings_check_default), current,
            getString(R.string.settings_action_default), () -> ImeSetup.makeDefault(this), true);
    }

    private void schemes(LinearLayout column) {
        AppEdition edition = AppEdition.current();
        JSONObject preferences = preferences();
        KeyboardScheme stored = preferences == null ? null : OnboardingChoices.storedScheme(preferences, edition);
        // 点下的方案先记下、立即显示为已选，偏好可读后才写进去（OnboardingChoices），所以词库还在准备时也能选。
        KeyboardScheme pending = OnboardingChoices.pendingScheme(this);
        KeyboardScheme current = OnboardingChoicePolicy.displayedScheme(pending, stored);
        // 双拼沿用已经选的双拼方案，只有第一次选才落在小鹤，与设计的「默认小鹤」一致。
        KeyboardScheme shuangpin = OnboardingChoicePolicy.shuangpinCard(pending, stored);
        // 五笔同理沿用已选的版本（选五笔不改 `wubi_profile`），说明文字照实写出当前是 86 还是 98。
        boolean wubi98 = preferences != null && KeyboardScheme.WUBI_98.equals(
            KeyboardScheme.normalizedWubiProfile(InputViewValuePolicy.textOr(
                preferences, "wubi_profile", KeyboardScheme.WUBI_86)));
        SchemeCard[] all = {
            new SchemeCard("全拼 26 键", "最常用，完整拼音", KeyboardScheme.QUANPIN),
            new SchemeCard("全拼 9 键", "单手更顺手", KeyboardScheme.QUANPIN_NINE_KEY),
            new SchemeCard("双拼", "每字两键 · 默认小鹤", shuangpin),
            new SchemeCard("五笔", wubi98 ? "形码 · 当前 98 版" : "形码 · 默认 86 版", KeyboardScheme.WUBI),
        };
        // 说明里的简称与上面的卡片一一对应，只列本版本有的那几张。
        String[] names = {"全拼", "9 键", "双拼", "五笔"};
        java.util.List<SchemeCard> cards = new java.util.ArrayList<>(all.length);
        java.util.List<String> offered = new java.util.ArrayList<>(all.length);
        for (int index = 0; index < all.length; index++) {
            if (!all[index].scheme().offeredBy(edition)) continue;
            cards.add(all[index]);
            offered.add(names[index]);
        }
        String listed = offered.size() == 1 ? offered.get(0)
            : String.join("、", offered.subList(0, offered.size() - 1)) + "和" + offered.get(offered.size() - 1);
        header(column, R.drawable.ic_ms_text_fields, ordinal() + " · 随时可以改", "选一套输入方案",
            listed + "用的是同一套引擎，词库和自造词通用。之后随时可以在「设置 → 输入」里换。");
        for (int index = 0; index < cards.size(); index++) {
            schemeCard(column, cards.get(index), cards.get(index).scheme() == current, index == 0 ? 6 : 10);
        }
        if (note != null) {
            footnote(column, note);
            return;
        }
        switch (footnoteState(pending != null)) {
            case SAVING -> footnote(column, "正在保存…");
            case READING -> footnote(column, "正在读取当前方案…");
            case FAILED -> preparationFailed(column, "选好的方案");
            case CHOSEN_WAITING -> footnote(column, "已记下，词库准备好后自动保存。");
            case WAITING -> footnote(column, "词库还在准备，可以先选好，准备好后自动保存。");
            case NONE -> { }
        }
    }

    private void translation(LinearLayout column) {
        header(column, R.drawable.ic_onboarding_translate, ordinal() + " · 水杉的特点", "候选下方就是译文",
            "打开后，每个候选词下面会多一行小字的英文释义，来自随应用打包的离线词典，不联网。");
        JSONObject preferences = preferences();
        // 与方案页一样：拨动后先记下、立即按它显示，偏好可读后再写进去。
        Boolean pending = OnboardingChoices.pendingGloss(this);
        boolean on = pending != null ? pending
            : preferences != null && preferences.optBoolean(OnboardingChoices.GLOSS, false);

        // A still of the candidate strip, drawn from the design's sample: what the switch below changes, before anyone has to open a text field to see it.
        LinearLayout strip = Ui.row(this);
        Ui.setSymmetricPaddingDp(strip, this, 10, 12);
        ViewPolicy.setBackground(strip, Ui.rounded(Ui.accentSoft(this), Ui.dp(this, 20)));
        ViewPolicy.setImportantForAccessibility(strip,
            View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        String[][] samples = {{"候选", "candidate"}, {"后选", "choice"}, {"侯选", "option"}, {"候", "wait"}};
        for (int index = 0; index < samples.length; index++) {
            LinearLayout cell = Ui.column(this);
            ViewPolicy.setCenteredHorizontally(cell);
            Ui.setHorizontalPaddingDp(cell, this, 10);
            TextView word = Ui.label(this, samples[index][0], 19, index == 0 ? Ui.accent(this) : Ui.text(this));
            if (index == 0) ViewPolicy.setTypefaceStyle(word, 600);
            cell.addView(word);
            if (on) cell.addView(Ui.label(this, samples[index][1], 11, Ui.subText(this)));
            strip.addView(cell);
        }
        column.addView(strip, Ui.matchWidth(this, 6));

        LinearLayout row = Ui.row(this);
        ViewPolicy.setCenteredVertically(row);
        Ui.setSymmetricPaddingDp(row, this, 14, 12);
        ViewPolicy.setBackground(row, Ui.rounded(Ui.card(this), Ui.dp(this, 20)));
        TextView label = Ui.label(this, "显示译文", 16, Ui.text(this));
        row.addView(label, Ui.weightWrap(1f));
        MaterialSwitch toggle = new MaterialSwitch(this);
        toggle.setChecked(on);
        toggle.setContentDescription("显示译文");
        toggle.setOnCheckedChangeListener((button, checked) -> {
            if (checked != on) chooseGloss(checked);
        });
        row.addView(toggle);
        ViewPolicy.bindClick(row, toggle::toggle);
        column.addView(row, Ui.matchWidth(this, 12));

        if (note != null) {
            footnote(column, note);
            return;
        }
        switch (footnoteState(pending != null)) {
            case SAVING -> footnote(column, "正在保存…");
            case READING -> footnote(column, "正在读取当前设置…");
            case FAILED -> preparationFailed(column, "设好的开关");
            case CHOSEN_WAITING -> footnote(column, "已记下，词库准备好后自动保存。");
            case WAITING -> footnote(column, "词库还在准备，可以先设好，准备好后自动保存。");
            case NONE -> footnote(column, "联网翻译默认关闭，要在「设置 → 词库」里单独开启，并会说明发送什么。");
        }
    }

    /** 方案页和译文页脚注的情况，见 {@link OnboardingChoicePolicy#footnote}。 */
    private OnboardingChoicePolicy.Footnote footnoteState(boolean pending) {
        return OnboardingChoicePolicy.footnote(preferences() != null, loaded,
            FirstRunPreparation.state() == FirstRunPreparation.State.FAILED, pending);
    }

    /** 词库准备失败时的脚注：写出原因（出问题的多是别人手里的手机，没法看 logcat），点它重试；选择照样记着，重试成功后自动保存。 */
    private void preparationFailed(LinearLayout column, String subject) {
        String reason = FirstRunPreparation.failure();
        TextView view = footnote(column, "词库准备失败" + (reason.isEmpty() ? "" : "：" + reason)
            + "。点这里重试，" + subject + "会在准备好后自动保存。");
        ViewPolicy.setTextColor(view, Ui.accent(this));
        ViewPolicy.bindClick(view, () -> {
            FirstRunPreparation.retry(this);
            render(false);
        });
    }

    private void sync(LinearLayout column) {
        String body = account == SignIn.State.SIGNED_IN
            ? "你已登录。打开「我的 → 云同步」后，词库、自造词、皮肤和云剪贴板会在手机、平板和电脑之间同步。"
            : "登录后词库、自造词、皮肤和云剪贴板会在手机、平板和电脑之间同步。日常输入不需要登录。";
        header(column, R.drawable.ic_ms_sync, "最后一步", "登录后多端同步", body);
        perk(column, R.drawable.ic_ms_menu_book, "词库和自造词", 6);
        perk(column, R.drawable.ic_ms_palette, "皮肤与主题", 10);
        perk(column, R.drawable.ic_ms_content_paste, "云剪贴板", 10);
        if (note != null) footnote(column, note);
    }

    // ---- sign-in ----

    /** Ask once whether the last step can offer sign-in; the answer may need the backend, so it is read off the main thread. */
    private void probeAccount() {
        Context context = getApplicationContext();
        offMainThread(() -> {
            SignIn.State state = SignIn.state(context);
            runOnUiThread(() -> {
                if (isFinishing() || isDestroyed()) return;
                account = state;
                if (page == pages - 1) render(false);
            });
        }, () -> runOnUiThread(() -> {
            if (isFinishing() || isDestroyed()) return;
            account = SignIn.State.ABSENT;
            if (page == pages - 1) render(false);
        }));
    }

    private void signIn() {
        if (signingIn) return;
        signingIn = true;
        note = "正在登录…";
        render(false);
        SignIn.start(this, failure -> {
            if (isFinishing() || isDestroyed()) return;
            signingIn = false;
            if (failure.isEmpty()) {
                account = SignIn.State.SIGNED_IN;
                note = null;
            } else if (LoginSheet.CANCELLED.equals(failure)) {
                declined = true;
                note = null;
            } else {
                note = failure;
            }
            render(false);
        });
    }

    // ---- writes ----

    private void chooseScheme(KeyboardScheme scheme) {
        OnboardingChoices.rememberScheme(this, scheme);
        note = null;
        render(false);
        syncChoices();
    }

    private void chooseGloss(boolean on) {
        OnboardingChoices.rememberGloss(this, on);
        note = null;
        render(false);
        syncChoices();
    }

    /**
     * 读一次偏好，有待保存的选择就写进去，再按结果重画方案页和译文页。
     *
     * <p>读写都在 {@link OnboardingChoices} 的同一条工作线程上排队，结果按提交顺序回到主线程，后发的读取不会被先发的覆盖。页面已经关掉时结果丢弃，但写入照样完成：选择不跟页面走。读不到时保留上一次读到的快照。
     */
    private void syncChoices() {
        OnboardingChoices.sync(this, result -> {
            if (isFinishing() || isDestroyed()) return;
            if (result.snapshot() != null) snapshot = result.snapshot();
            loaded = true;
            boolean choices = steps[page] == STEP_SCHEMES || steps[page] == STEP_TRANSLATION;
            if (result.outcome() == OnboardingChoices.Outcome.ERROR) {
                // 意外的异常不代表偏好读不到：页面手上的旧快照仍可读，不处理就会一直停在「正在保存…」。隔一会儿再试几次，仍不行就说明情况，选择留着下次启动再写。
                if (syncRetries < SYNC_RETRY_LIMIT) {
                    syncRetries++;
                    retryHandler.postDelayed(this::syncChoices, SYNC_RETRY_DELAY_MS);
                } else if (choices) {
                    note = "暂时保存不了，已记下选择，下次打开水杉时再保存";
                }
            } else {
                syncRetries = 0;
            }
            if (choices && result.outcome() == OnboardingChoices.Outcome.FAILED) {
                note = steps[page] == STEP_SCHEMES ? "保存失败，键盘保留当前方案" : "保存失败，保留原来的设置";
            }
            if (choices) render(false);
        });
    }

    @Nullable private JSONObject preferences() {
        return snapshot == null ? null : snapshot.optJSONObject("preferences");
    }

    // ---- pieces ----

    /** Glyph, kicker, title and body: the design's 48dp glyph box, 13sp accent kicker, 32sp regular title and 16sp body, 14dp apart. */
    private void header(LinearLayout column, @DrawableRes int icon, String kicker, String title,
            String body) {
        ImageView glyph = Ui.decorativeIcon(this, icon, Ui.accent(this));
        glyph.setScaleType(ImageView.ScaleType.FIT_START);
        Ui.hideFromAccessibility(glyph);
        column.addView(glyph, Ui.squareParams(this, 36));

        TextView kick = Ui.label(this, kicker, 13, Ui.accent(this));
        ViewPolicy.setTypefaceStyle(kick, 600);
        kick.setLetterSpacing(0.04f);
        column.addView(kick, Ui.matchWidth(this, 14 + 6));

        TextView heading = Ui.label(this, title, 32, Ui.text(this));
        ViewPolicy.setLineSpacing(heading, 0, 1.1f);
        heading.setAccessibilityHeading(true);
        column.addView(heading, Ui.matchWidth(this, 14));

        TextView line = Ui.label(this, body, 16, Ui.subText(this));
        ViewPolicy.setLineSpacing(line, 0, 1.35f);
        column.addView(line, Ui.matchWidth(this, 14));
    }

    private LinearLayout card(LinearLayout column, int top) {
        LinearLayout card = Ui.verticalCard(this, 20);
        column.addView(card, Ui.matchWidth(this, 14 + top));
        return card;
    }

    /** One setup fact: a disc that is a tick or a warning, the fact, and the fix when it is not done yet. */
    private void check(LinearLayout card, String label, boolean done, String action,
            Runnable fix, boolean divider) {
        if (divider) {
            View line = new View(this);
            ViewPolicy.setBackgroundColor(line, Ui.hairline(this));
            card.addView(line, Ui.matchWidthHeightPx(
                BoundsPolicy.bounded(Ui.dp(this, 1) / 2, 1, Integer.MAX_VALUE)));
        }
        LinearLayout row = Ui.row(this);
        ViewPolicy.setCenteredVertically(row);
        Ui.setMinimumHeightDp(row, this, Ui.COMPACT_ROW_MIN_HEIGHT);
        Ui.setPaddingDp(row, this, 14, 6, 8, 6);

        TextView mark = Ui.label(this, done ? "✓" : "!", 13,
            done ? Ui.onAccent(this) : 0xFFFFFFFF);
        ViewPolicy.setCentered(mark);
        // 字形画在固定 dp 的圆里，跟圆一起按 dp 定大小；按 sp 时系统字体一调大，对勾就被圆的边界切掉。
        ViewPolicy.setTextSizeDp(mark, 13);
        Ui.applyStatusMark(mark, this, done);
        row.addView(mark, Ui.squareParams(this, 24));

        TextView text = Ui.label(this, label, 16, Ui.text(this));
        LinearLayout.LayoutParams textParams = Ui.weightWrap(1f);
        textParams.setMarginStart(Ui.dp(this, 12));
        row.addView(text, textParams);
        text.setContentDescription(label + (done ? "，已完成" : "，未完成"));

        if (!done) {
            TextView button = Ui.label(this, action, 15, Ui.accent(this));
            ViewPolicy.setCentered(button);
            Ui.setHorizontalPaddingDp(button, this, 8);
            android.util.TypedValue ripple = new android.util.TypedValue();
            getTheme().resolveAttribute(android.R.attr.selectableItemBackground, ripple, true);
            button.setBackgroundResource(ripple.resourceId);
            ViewPolicy.bindClick(button, fix);
            row.addView(button, Ui.wrapHeight(this, 40));
        }
        card.addView(row);
    }

    /** A scheme card: label and supporting line, a radio disc on the right, a 2dp accent ring when chosen. */
    private void schemeCard(LinearLayout column, SchemeCard option, boolean selected, int top) {
        LinearLayout card = Ui.row(this);
        ViewPolicy.setCenteredVertically(card);
        Ui.setSymmetricPaddingDp(card, this, 16, 14);
        GradientDrawable face = selected
            ? Ui.outlined(Ui.card(this), Ui.dp(this, 20), Ui.dp(this, 2), Ui.accent(this))
            : Ui.rounded(Ui.card(this), Ui.dp(this, 20));
        ViewPolicy.setBackground(card, face);

        LinearLayout text = Ui.column(this);
        TextView heading = Ui.label(this, option.label(), 16, Ui.text(this));
        ViewPolicy.setTypefaceStyle(heading, 600);
        text.addView(heading);
        TextView detail = Ui.label(this, option.detail(), 13, Ui.subText(this));
        LinearLayout.LayoutParams detailParams = Ui.wrap();
        detailParams.topMargin = Ui.dp(this, 2);
        text.addView(detail, detailParams);
        card.addView(text, Ui.weightWrap(1f));

        View radio = new View(this);
        GradientDrawable dot = Ui.circleOutlined(selected ? Ui.page(this) : 0,
            selected ? Ui.dp(this, 6) : Ui.atLeastOnePx(this, 1.5f),
            selected ? Ui.accent(this) : Ui.subText(this));
        ViewPolicy.setBackground(radio, dot);
        LinearLayout.LayoutParams radioParams = Ui.squareParams(this, 22);
        radioParams.setMarginStart(Ui.dp(this, 12));
        card.addView(radio, radioParams);

        card.setContentDescription(option.label() + "，" + option.detail()
            + (selected ? "，已选择" : "，未选择"));
        // 偏好还读不到时也能点：选择先记下，偏好可读后再写（OnboardingChoices）。
        ViewPolicy.bindOptionalClick(card, selected ? null : () -> chooseScheme(option.scheme()));
        ViewPolicy.setClickable(card, true);
        column.addView(card, Ui.matchWidth(this, top));
    }

    private void perk(LinearLayout column, @DrawableRes int icon, String label, int top) {
        LinearLayout row = Ui.row(this);
        ViewPolicy.setCenteredVertically(row);
        Ui.setSymmetricPaddingDp(row, this, 14, 12);
        ViewPolicy.setBackground(row, Ui.rounded(Ui.card(this), Ui.dp(this, 20)));
        ImageView badge = Ui.decorativeIcon(this, icon, Ui.accent(this));
        Ui.setSymmetricPaddingDp(badge, this, 7, 7);
        ViewPolicy.setBackground(badge, Ui.rounded(Ui.accentSoft(this), Ui.dp(this, 9)));
        Ui.hideFromAccessibility(badge);
        row.addView(badge, Ui.squareParams(this, 32));
        TextView text = Ui.label(this, label, 15, Ui.text(this));
        LinearLayout.LayoutParams textParams = Ui.weightWrap(1f);
        textParams.setMarginStart(Ui.dp(this, 12));
        row.addView(text, textParams);
        column.addView(row, Ui.matchWidth(this, 14 + top - 10));
    }

    private TextView footnote(LinearLayout column, String message) {
        TextView view = Ui.label(this, message, 13, Ui.subText(this));
        ViewPolicy.setPoliteLiveRegion(view);
        column.addView(view, Ui.matchWidth(this, 14));
        return view;
    }

    /**
     * 这一页的磁盘读写。
     *
     * <p>{@link HostTask} 只收 Fragment，而这是一个 Activity；这里要的只是「别在主线程上读盘」，所以起一条一次性的线程。A failure runs the fallback so the page never waits on a read that is not coming.
     */
    private void offMainThread(Runnable work, Runnable failed) {
        new Thread(() -> {
            try {
                work.run();
            } catch (RuntimeException | LinkageError error) {
                failed.run();
            }
        }, "msime-onboarding").start();
    }

}
