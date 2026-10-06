package app.msime.android.home;

import app.msime.android.KeyboardGeometry;
import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Typeface;
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
import app.msime.android.FirstRunPreparation;
import app.msime.android.KeyboardScheme;
import app.msime.android.R;
import app.msime.android.SchemePreferences;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.progressindicator.LinearProgressIndicator;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 新手引导：四步，从「把键盘加进系统」走到「登录能多得到什么」。
 *
 * <p>The design's Android flow: a progress bar rather than a counter, an accent glyph, a kicker, a large regular-weight title and a body line, then the step's own content. The left button reads 跳过 on the first step and 上一步 after it; a horizontal swipe moves between steps too.
 *
 * <p>Every control on these pages changes something real. The scheme cards write through {@link SchemePreferences#withScheme}, the same mapping the settings tab's scheme picker uses; the 显示译文 switch is the offline English gloss preference the keyboard reads. The last step's 登录 opens {@link LoginSheet} (Apple / Google / 邮箱，按后端接受的方式), shown only when sign-in is offered; after a successful sign-in, after the sheet was closed without signing in, or when sign-in is not offered at all, it reads 开始使用. 设计没有「稍后再说」（与「跳过」重复，P12），所以这里也没有。
 */
public final class OnboardingActivity extends AppCompatActivity {
    private static final String STORE = "msime_onboarding_v1";
    private static final String SEEN = "seen";
    private static final int STEP_ENABLE = 0;
    private static final int STEP_SCHEMES = 1;
    private static final int STEP_TRANSLATION = 2;
    private static final int STEP_SYNC = 3;
    private static final String STATE_PAGE = "onboarding-page";
    /** The keyboard reads this key for the per-candidate English line; the core's default is off. */
    private static final String GLOSS = "candidate_english_gloss";

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
    private boolean saving;
    /** What the last write on this page did, shown under the page's controls until the page changes. */
    @Nullable private String note;
    /** Whether the last step can offer sign-in; null until the off-thread check answers, which reads as not offered. */
    @Nullable private SignIn.State account;
    private boolean signingIn;
    /** 在最后一步打开过登录面板又没登录就关掉了：主按钮改为「开始使用」，不再反复弹出面板（设计去掉了「稍后再说」，P12）。 */
    private boolean declined;
    private OnBackPressedCallback back;
    private GestureDetector swipe;
    /** 首次安装时，引导页打开时词库还在准备，准备完成前读不到偏好设置；如果不在完成后重新读取，方案页和释义页会一直显示「词库还在准备」。 */
    private final FirstRunPreparation.Listener preparation = status -> {
        if (status == FirstRunPreparation.State.READY && preferences() == null && !isFinishing() && !isDestroyed()) reload();
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
        findViewById(R.id.onboarding_skip).setOnClickListener(ignored -> finishFlow());
        findViewById(R.id.onboarding_previous).setOnClickListener(ignored -> go(page - 1));
        findViewById(R.id.onboarding_next).setOnClickListener(ignored -> {
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
        reload();
        FirstRunPreparation.observe(preparation);
        probeAccount();
    }

    @Override protected void onDestroy() {
        FirstRunPreparation.stopObserving(preparation);
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
        findViewById(R.id.onboarding_skip).setVisibility(page == 0 ? View.VISIBLE : View.GONE);
        findViewById(R.id.onboarding_previous).setVisibility(page == 0 ? View.GONE : View.VISIBLE);
        boolean offer = page == pages - 1 && account == SignIn.State.OFFERED && !declined;
        MaterialButton next = findViewById(R.id.onboarding_next);
        next.setText(page < pages - 1 ? R.string.onboarding_next
            : offer ? R.string.onboarding_sign_in : R.string.onboarding_done);
        next.setEnabled(!signingIn);
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
        KeyboardScheme current = preferences == null ? null : KeyboardScheme.fromPreferences(
            preferences.optString("scheme", edition.defaultScheme()),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"), edition);
        // 双拼 keeps whichever double-pinyin profile is already chosen; only a first pick lands on 小鹤, as the design's 默认小鹤 says.
        KeyboardScheme shuangpin = current != null && current.shuangpinProfile() != null
            ? current : KeyboardScheme.XIAOHE;
        // 五笔同理沿用已选的版本（选五笔不改 `wubi_profile`），说明文字照实写出当前是 86 还是 98。
        boolean wubi98 = preferences != null && KeyboardScheme.WUBI_98.equals(
            KeyboardScheme.normalizedWubiProfile(preferences.optString("wubi_profile", KeyboardScheme.WUBI_86)));
        SchemeCard[] all = {
            new SchemeCard("全拼 26 键", "最常用，完整拼音", KeyboardScheme.QUANPIN),
            new SchemeCard("全拼 9 键", "单手更顺手", KeyboardScheme.QUANPIN_NINE_KEY),
            new SchemeCard("双拼", "每字两键 · 默认小鹤", shuangpin),
            new SchemeCard("五笔", wubi98 ? "形码 · 当前 98 版" : "形码 · 默认 86 版", KeyboardScheme.WUBI),
        };
        // 说明里的简称与上面的卡片一一对应，只列本版本有的那几张。
        String[] names = {"全拼", "9 键", "双拼", "五笔"};
        java.util.List<SchemeCard> cards = new java.util.ArrayList<>();
        java.util.List<String> offered = new java.util.ArrayList<>();
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
        if (preferences == null) {
            footnote(column, loaded ? "词库还在准备，暂时不能保存方案。稍后可以在「设置 → 输入」里选。"
                : "正在读取当前方案…");
        } else if (note != null) {
            footnote(column, note);
        }
    }

    private void translation(LinearLayout column) {
        header(column, R.drawable.ic_onboarding_translate, ordinal() + " · 水杉的特点", "候选下方就是译文",
            "打开后，每个候选词下面会多一行小字的英文释义，来自随应用打包的离线词典，不联网。");
        JSONObject preferences = preferences();
        boolean on = preferences != null && preferences.optBoolean(GLOSS, false);

        // A still of the candidate strip, drawn from the design's sample: what the switch below changes, before anyone has to open a text field to see it.
        LinearLayout strip = new LinearLayout(this);
        strip.setOrientation(LinearLayout.HORIZONTAL);
        strip.setPadding(Ui.dp(this, 10), Ui.dp(this, 12), Ui.dp(this, 10), Ui.dp(this, 12));
        strip.setBackground(Ui.rounded(Ui.accentSoft(this), Ui.dp(this, 20)));
        strip.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        String[][] samples = {{"候选", "candidate"}, {"后选", "choice"}, {"侯选", "option"}, {"候", "wait"}};
        for (int index = 0; index < samples.length; index++) {
            LinearLayout cell = new LinearLayout(this);
            cell.setOrientation(LinearLayout.VERTICAL);
            cell.setGravity(Gravity.CENTER_HORIZONTAL);
            cell.setPadding(Ui.dp(this, 10), 0, Ui.dp(this, 10), 0);
            TextView word = text(samples[index][0], 19, index == 0 ? Ui.accent(this) : Ui.text(this));
            if (index == 0) word.setTypeface(Typeface.create(Typeface.DEFAULT, 600, false));
            cell.addView(word);
            if (on) cell.addView(text(samples[index][1], 11, Ui.subText(this)));
            strip.addView(cell);
        }
        column.addView(strip, Ui.matchWidth(this, 6));

        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setPadding(Ui.dp(this, 14), Ui.dp(this, 12), Ui.dp(this, 14), Ui.dp(this, 12));
        row.setBackground(Ui.rounded(Ui.card(this), Ui.dp(this, 20)));
        TextView label = text("显示译文", 16, Ui.text(this));
        row.addView(label, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        MaterialSwitch toggle = new MaterialSwitch(this);
        toggle.setChecked(on);
        toggle.setEnabled(preferences != null && !saving);
        toggle.setContentDescription("显示译文");
        toggle.setOnCheckedChangeListener((button, checked) -> {
            if (checked != on) save(GLOSS, checked);
        });
        row.addView(toggle);
        row.setOnClickListener(ignored -> { if (toggle.isEnabled()) toggle.toggle(); });
        column.addView(row, Ui.matchWidth(this, 12));

        if (preferences == null) {
            footnote(column, loaded ? "词库还在准备，暂时不能保存这个开关。稍后可以在「设置 → 词库」里打开。"
                : "正在读取当前设置…");
        } else {
            footnote(column, note != null ? note
                : "联网翻译默认关闭，要在「设置 → 词库」里单独开启，并会说明发送什么。");
        }
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

    private void selectScheme(KeyboardScheme scheme) {
        JSONObject current = snapshot;
        if (current == null || saving) return;
        JSONObject pending = SchemePreferences.withScheme(current, scheme, null);
        if (pending == null) {
            note = "切换失败，保留当前方案";
            render(false);
            return;
        }
        write(pending, "保存失败，键盘保留当前方案");
    }

    private void save(String key, Object value) {
        JSONObject current = snapshot;
        if (current == null || saving) return;
        JSONObject pending;
        try {
            pending = new JSONObject(current.toString());
            pending.getJSONObject("preferences").put(key, value);
        } catch (JSONException error) {
            note = "保存失败，保留原来的设置";
            render(false);
            return;
        }
        write(pending, "保存失败，保留原来的设置");
    }

    /** Save a snapshot and read it back, so the next change starts from what was actually stored rather than from what was sent. */
    private void write(JSONObject pending, String failure) {
        saving = true;
        note = "正在保存…";
        render(false);
        Context context = getApplicationContext();
        offMainThread(() -> {
            JSONObject saved = HostStore.savePreferences(context, pending);
            JSONObject fresh = HostStore.loadPreferences(context);
            runOnUiThread(() -> {
                if (isFinishing() || isDestroyed()) return;
                saving = false;
                if (fresh != null) snapshot = fresh;
                note = saved == null ? failure : null;
                render(false);
            });
        }, () -> runOnUiThread(() -> {
            if (isFinishing() || isDestroyed()) return;
            saving = false;
            note = failure;
            render(false);
        }));
    }

    private void reload() {
        Context context = getApplicationContext();
        offMainThread(() -> {
            JSONObject value = HostStore.loadPreferences(context);
            runOnUiThread(() -> {
                if (isFinishing() || isDestroyed()) return;
                snapshot = value;
                loaded = true;
                if (steps[page] == STEP_SCHEMES || steps[page] == STEP_TRANSLATION) render(false);
            });
        }, () -> runOnUiThread(() -> {
            if (isFinishing() || isDestroyed()) return;
            loaded = true;
            if (steps[page] == STEP_SCHEMES || steps[page] == STEP_TRANSLATION) render(false);
        }));
    }

    @Nullable private JSONObject preferences() {
        return snapshot == null ? null : snapshot.optJSONObject("preferences");
    }

    // ---- pieces ----

    /** Glyph, kicker, title and body: the design's 48dp glyph box, 13sp accent kicker, 32sp regular title and 16sp body, 14dp apart. */
    private void header(LinearLayout column, @DrawableRes int icon, String kicker, String title,
            String body) {
        ImageView glyph = new ImageView(this);
        glyph.setImageResource(icon);
        glyph.setImageTintList(ColorStateList.valueOf(Ui.accent(this)));
        glyph.setScaleType(ImageView.ScaleType.FIT_START);
        glyph.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        column.addView(glyph, new LinearLayout.LayoutParams(Ui.dp(this, 36), Ui.dp(this, 36)));

        TextView kick = text(kicker, 13, Ui.accent(this));
        kick.setTypeface(Typeface.create(Typeface.DEFAULT, 600, false));
        kick.setLetterSpacing(0.04f);
        column.addView(kick, Ui.matchWidth(this, 14 + 6));

        TextView heading = text(title, 32, Ui.text(this));
        heading.setLineSpacing(0, 1.1f);
        heading.setAccessibilityHeading(true);
        column.addView(heading, Ui.matchWidth(this, 14));

        TextView line = text(body, 16, Ui.subText(this));
        line.setLineSpacing(0, 1.35f);
        column.addView(line, Ui.matchWidth(this, 14));
    }

    private LinearLayout card(LinearLayout column, int top) {
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setBackground(Ui.rounded(Ui.card(this), Ui.dp(this, 20)));
        column.addView(card, Ui.matchWidth(this, 14 + top));
        return card;
    }

    /** One setup fact: a disc that is a tick or a warning, the fact, and the fix when it is not done yet. */
    private void check(LinearLayout card, String label, boolean done, String action,
            Runnable fix, boolean divider) {
        if (divider) {
            View line = new View(this);
            line.setBackgroundColor(Ui.hairline(this));
            card.addView(line, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, Math.max(1, Ui.dp(this, 1) / 2)));
        }
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setMinimumHeight(Ui.dp(this, 52));
        row.setPadding(Ui.dp(this, 14), Ui.dp(this, 6), Ui.dp(this, 8), Ui.dp(this, 6));

        TextView mark = new TextView(this);
        mark.setGravity(Gravity.CENTER);
        mark.setTextSize(13);
        mark.setText(done ? "✓" : "!");
        mark.setTextColor(done ? Ui.onAccent(this) : 0xFFFFFFFF);
        GradientDrawable disc = Ui.circle(done ? Ui.accent(this) : Ui.color(this, R.attr.msWarn));
        mark.setBackground(disc);
        mark.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        row.addView(mark, new LinearLayout.LayoutParams(Ui.dp(this, 24), Ui.dp(this, 24)));

        TextView text = text(label, 16, Ui.text(this));
        LinearLayout.LayoutParams textParams =
            new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1);
        textParams.setMarginStart(Ui.dp(this, 12));
        row.addView(text, textParams);
        text.setContentDescription(label + (done ? "，已完成" : "，未完成"));

        if (!done) {
            TextView button = text(action, 15, Ui.accent(this));
            button.setGravity(Gravity.CENTER);
            button.setPadding(Ui.dp(this, 8), 0, Ui.dp(this, 8), 0);
            android.util.TypedValue ripple = new android.util.TypedValue();
            getTheme().resolveAttribute(android.R.attr.selectableItemBackground, ripple, true);
            button.setBackgroundResource(ripple.resourceId);
            button.setOnClickListener(ignored -> fix.run());
            row.addView(button, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, Ui.dp(this, 40)));
        }
        card.addView(row);
    }

    /** A scheme card: label and supporting line, a radio disc on the right, a 2dp accent ring when chosen. */
    private void schemeCard(LinearLayout column, SchemeCard option, boolean selected, int top) {
        boolean usable = snapshot != null && !saving;
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.HORIZONTAL);
        card.setGravity(Gravity.CENTER_VERTICAL);
        card.setPadding(Ui.dp(this, 16), Ui.dp(this, 14), Ui.dp(this, 16), Ui.dp(this, 14));
        GradientDrawable face = Ui.rounded(Ui.card(this), Ui.dp(this, 20));
        if (selected) face.setStroke(Ui.dp(this, 2), Ui.accent(this));
        card.setBackground(face);

        LinearLayout text = new LinearLayout(this);
        text.setOrientation(LinearLayout.VERTICAL);
        TextView heading = text(option.label(), 16, Ui.text(this));
        heading.setTypeface(Typeface.create(Typeface.DEFAULT, 600, false));
        text.addView(heading);
        TextView detail = text(option.detail(), 13, Ui.subText(this));
        LinearLayout.LayoutParams detailParams = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        detailParams.topMargin = Ui.dp(this, 2);
        text.addView(detail, detailParams);
        card.addView(text, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));

        View radio = new View(this);
        GradientDrawable dot = new GradientDrawable();
        dot.setShape(GradientDrawable.OVAL);
        if (selected) {
            dot.setColor(Ui.page(this));
            dot.setStroke(Ui.dp(this, 6), Ui.accent(this));
        } else {
            dot.setColor(0);
            dot.setStroke(Ui.atLeastOnePx(this, 1.5f),
                Ui.subText(this));
        }
        radio.setBackground(dot);
        LinearLayout.LayoutParams radioParams = new LinearLayout.LayoutParams(Ui.dp(this, 22), Ui.dp(this, 22));
        radioParams.setMarginStart(Ui.dp(this, 12));
        card.addView(radio, radioParams);

        card.setContentDescription(option.label() + "，" + option.detail()
            + (selected ? "，已选择" : "，未选择"));
        card.setEnabled(usable);
        card.setAlpha(usable || selected ? 1f : 0.6f);
        card.setOnClickListener(usable && !selected ? ignored -> selectScheme(option.scheme()) : null);
        card.setClickable(usable);
        column.addView(card, Ui.matchWidth(this, top));
    }

    private void perk(LinearLayout column, @DrawableRes int icon, String label, int top) {
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setPadding(Ui.dp(this, 14), Ui.dp(this, 12), Ui.dp(this, 14), Ui.dp(this, 12));
        row.setBackground(Ui.rounded(Ui.card(this), Ui.dp(this, 20)));
        ImageView badge = new ImageView(this);
        badge.setImageResource(icon);
        badge.setImageTintList(ColorStateList.valueOf(Ui.accent(this)));
        badge.setPadding(Ui.dp(this, 7), Ui.dp(this, 7), Ui.dp(this, 7), Ui.dp(this, 7));
        badge.setBackground(Ui.rounded(Ui.accentSoft(this), Ui.dp(this, 9)));
        badge.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        row.addView(badge, new LinearLayout.LayoutParams(Ui.dp(this, 32), Ui.dp(this, 32)));
        TextView text = text(label, 15, Ui.text(this));
        LinearLayout.LayoutParams textParams =
            new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1);
        textParams.setMarginStart(Ui.dp(this, 12));
        row.addView(text, textParams);
        column.addView(row, Ui.matchWidth(this, 14 + top - 10));
    }

    private void footnote(LinearLayout column, String message) {
        TextView view = text(message, 13, Ui.subText(this));
        view.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        column.addView(view, Ui.matchWidth(this, 14));
    }

    private TextView text(String value, int size, int colour) {
        return Ui.label(this, value, size, colour);
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
