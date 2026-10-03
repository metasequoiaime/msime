package app.msime.android.home;

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
import androidx.core.content.ContextCompat;
import androidx.core.widget.NestedScrollView;
import app.msime.android.FirstRunPreparation;
import app.msime.android.KeyboardScheme;
import app.msime.android.R;
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
 * <p>Every control on these pages changes something real. The scheme cards write the same preferences the settings tab's scheme picker writes, through the same mapping; the 显示译文 switch is the offline English gloss preference the keyboard reads. The last step's 登录 is the same Google sign-in 我的 offers ({@link SignIn}), shown only when that is offered: then the primary button reads 登录 with 稍后再说 beside it, and after a successful sign-in, or when sign-in is not offered at all, it reads 开始使用.
 */
public final class OnboardingActivity extends AppCompatActivity {
    private static final String STORE = "msime_onboarding_v1";
    private static final String SEEN = "seen";
    private static final int PAGES = 4;
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

    private int page;
    @Nullable private JSONObject snapshot;
    private boolean loaded;
    private boolean saving;
    /** What the last write on this page did, shown under the page's controls until the page changes. */
    @Nullable private String note;
    /** Whether the last step can offer sign-in; null until the off-thread check answers, which reads as not offered. */
    @Nullable private SignIn.State account;
    private boolean signingIn;
    private OnBackPressedCallback back;
    private GestureDetector swipe;
    /** On a first install the onboarding opens while the dictionary is still being prepared, and the preferences are unreadable until it is done; without asking again the scheme and gloss pages say 词库还在准备 for good. */
    private final FirstRunPreparation.Listener preparation = status -> {
        if (status == FirstRunPreparation.State.READY && preferences() == null && !isFinishing() && !isDestroyed()) reload();
    };

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        setContentView(R.layout.activity_onboarding);
        if (state != null) page = Math.max(0, Math.min(PAGES - 1, state.getInt(STATE_PAGE, 0)));
        findViewById(R.id.onboarding_skip).setOnClickListener(ignored -> finishFlow());
        findViewById(R.id.onboarding_previous).setOnClickListener(ignored -> go(page - 1));
        findViewById(R.id.onboarding_next).setOnClickListener(ignored -> {
            if (page < PAGES - 1) go(page + 1);
            else if (account == SignIn.State.OFFERED) signIn();
            else finishFlow();
        });
        findViewById(R.id.onboarding_later).setOnClickListener(ignored -> finishFlow());
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
                if (Math.abs(dx) < pixels(50) || Math.abs(dx) < Math.abs(dy) * 1.5f) return false;
                if (dx < 0 && page < PAGES - 1) go(page + 1);
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
        if (target < 0 || target >= PAGES || target == page) return;
        page = target;
        note = null;
        render(true);
    }

    private void render(boolean animate) {
        ((LinearProgressIndicator) findViewById(R.id.onboarding_progress))
            .setProgressCompat(page + 1, animate);
        findViewById(R.id.onboarding_skip).setVisibility(page == 0 ? View.VISIBLE : View.GONE);
        findViewById(R.id.onboarding_previous).setVisibility(page == 0 ? View.GONE : View.VISIBLE);
        boolean offer = page == PAGES - 1 && account == SignIn.State.OFFERED;
        MaterialButton next = findViewById(R.id.onboarding_next);
        next.setText(page < PAGES - 1 ? R.string.onboarding_next
            : offer ? R.string.onboarding_sign_in : R.string.onboarding_done);
        next.setEnabled(!signingIn);
        findViewById(R.id.onboarding_later).setVisibility(offer ? View.VISIBLE : View.GONE);
        back.setEnabled(page > 0);

        LinearLayout column = findViewById(R.id.onboarding_page);
        column.removeAllViews();
        switch (page) {
            case 0 -> enable(column);
            case 1 -> schemes(column);
            case 2 -> translation(column);
            default -> sync(column);
        }
        if (animate) {
            ((NestedScrollView) findViewById(R.id.onboarding_scroll)).scrollTo(0, 0);
            column.startAnimation(AnimationUtils.loadAnimation(this, R.anim.fade_through_in));
        }
    }

    // ---- steps ----

    private void enable(LinearLayout column) {
        header(column, R.drawable.ic_tab_keyboard, "第一步 · 约 30 秒", "把水杉加进键盘",
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
        header(column, R.drawable.ic_feature_scheme, "第二步 · 随时可以改", "选一套输入方案",
            "全拼、9 键、双拼和五笔都在这里，之后随时可以在「设置 → 输入」里换。");
        JSONObject preferences = preferences();
        KeyboardScheme current = preferences == null ? null : KeyboardScheme.fromPreferences(
            preferences.optString("scheme", "quanpin"),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"));
        // 双拼 keeps whichever double-pinyin profile is already chosen; only a first pick lands on 小鹤, as the design's 默认小鹤 says.
        KeyboardScheme shuangpin = current != null && current.shuangpinProfile() != null
            ? current : KeyboardScheme.XIAOHE;
        // 五笔同理沿用已选的版本（选五笔不改 `wubi_profile`），说明文字照实写出当前是 86 还是 98。
        boolean wubi98 = preferences != null && KeyboardScheme.WUBI_98.equals(
            KeyboardScheme.normalizedWubiProfile(preferences.optString("wubi_profile", KeyboardScheme.WUBI_86)));
        SchemeCard[] cards = {
            new SchemeCard("全拼 26 键", "最常用，完整拼音", KeyboardScheme.QUANPIN),
            new SchemeCard("全拼 9 键", "单手更顺手", KeyboardScheme.QUANPIN_NINE_KEY),
            new SchemeCard("双拼", "每字两键 · 默认小鹤", shuangpin),
            new SchemeCard("五笔", wubi98 ? "形码 · 当前 98 版" : "形码 · 默认 86 版", KeyboardScheme.WUBI),
        };
        for (int index = 0; index < cards.length; index++) {
            schemeCard(column, cards[index], cards[index].scheme() == current, index == 0 ? 6 : 10);
        }
        if (preferences == null) {
            footnote(column, loaded ? "词库还在准备，暂时不能保存方案。稍后可以在「设置 → 输入」里选。"
                : "正在读取当前方案…");
        } else if (note != null) {
            footnote(column, note);
        }
    }

    private void translation(LinearLayout column) {
        header(column, R.drawable.ic_onboarding_translate, "第三步 · 水杉的特点", "候选下方就是译文",
            "打开后，每个候选词下面会多一行小字的英文释义，来自随应用打包的离线词典，不联网。");
        JSONObject preferences = preferences();
        boolean on = preferences != null && preferences.optBoolean(GLOSS, false);

        // A still of the candidate strip, drawn from the design's sample: what the switch below changes, before anyone has to open a text field to see it.
        LinearLayout strip = new LinearLayout(this);
        strip.setOrientation(LinearLayout.HORIZONTAL);
        strip.setPadding(pixels(10), pixels(12), pixels(10), pixels(12));
        strip.setBackground(rounded(color(R.color.search_field), pixels(20)));
        strip.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        String[][] samples = {{"候选", "candidate"}, {"后选", "choice"}, {"侯选", "option"}, {"候", "wait"}};
        for (int index = 0; index < samples.length; index++) {
            LinearLayout cell = new LinearLayout(this);
            cell.setOrientation(LinearLayout.VERTICAL);
            cell.setGravity(Gravity.CENTER_HORIZONTAL);
            cell.setPadding(pixels(10), 0, pixels(10), 0);
            TextView word = text(samples[index][0], 19, index == 0 ? R.color.forest : R.color.ink);
            if (index == 0) word.setTypeface(Typeface.create(Typeface.DEFAULT, 600, false));
            cell.addView(word);
            if (on) cell.addView(text(samples[index][1], 11, R.color.text_secondary));
            strip.addView(cell);
        }
        column.addView(strip, blockParams(6));

        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setPadding(pixels(14), pixels(12), pixels(14), pixels(12));
        row.setBackground(rounded(color(R.color.surface), pixels(20)));
        TextView label = text("显示译文", 16, R.color.ink);
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
        column.addView(row, blockParams(12));

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
            ? "你已登录，云词库、云剪贴板和社区作品会跟着你的账号走。"
            : account == SignIn.State.OFFERED
                ? "登录后，云词库、云剪贴板和社区作品会跟着你的账号走。日常输入不需要登录，也可以稍后在「我的」里登录。"
                : "登录后，云词库、云剪贴板和社区作品会跟着你的账号走。日常输入不需要登录。";
        header(column, R.drawable.ic_onboarding_sync, "最后一步", "登录后多端同步", body);
        perk(column, R.drawable.ic_feature_dictionary, "云词库：个人词条与候选", 6);
        perk(column, R.drawable.ic_onboarding_clipboard, "云剪贴板：你明确添加的内容", 10);
        perk(column, R.drawable.ic_feature_skin, "社区作品：发布与收藏的皮肤和词库", 10);
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
                if (page == PAGES - 1) render(false);
            });
        }, () -> runOnUiThread(() -> {
            if (isFinishing() || isDestroyed()) return;
            account = SignIn.State.ABSENT;
            if (page == PAGES - 1) render(false);
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
        JSONObject pending = KeyboardSheets.withScheme(current, scheme);
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
                if (page == 1 || page == 2) render(false);
            });
        }, () -> runOnUiThread(() -> {
            if (isFinishing() || isDestroyed()) return;
            loaded = true;
            if (page == 1 || page == 2) render(false);
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
        glyph.setImageTintList(ColorStateList.valueOf(color(R.color.forest)));
        glyph.setScaleType(ImageView.ScaleType.FIT_START);
        glyph.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        column.addView(glyph, new LinearLayout.LayoutParams(pixels(36), pixels(36)));

        TextView kick = text(kicker, 13, R.color.forest);
        kick.setTypeface(Typeface.create(Typeface.DEFAULT, 600, false));
        kick.setLetterSpacing(0.04f);
        column.addView(kick, blockParams(14 + 6));

        TextView heading = text(title, 32, R.color.ink);
        heading.setLineSpacing(0, 1.1f);
        heading.setAccessibilityHeading(true);
        column.addView(heading, blockParams(14));

        TextView line = text(body, 16, R.color.text_secondary);
        line.setLineSpacing(0, 1.35f);
        column.addView(line, blockParams(14));
    }

    private LinearLayout card(LinearLayout column, int top) {
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setBackground(rounded(color(R.color.surface), pixels(20)));
        column.addView(card, blockParams(14 + top));
        return card;
    }

    /** One setup fact: a disc that is a tick or a warning, the fact, and the fix when it is not done yet. */
    private void check(LinearLayout card, String label, boolean done, String action,
            Runnable fix, boolean divider) {
        if (divider) {
            View line = new View(this);
            line.setBackgroundColor(color(R.color.hairline));
            card.addView(line, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, Math.max(1, pixels(1) / 2)));
        }
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setMinimumHeight(pixels(52));
        row.setPadding(pixels(14), pixels(6), pixels(8), pixels(6));

        TextView mark = new TextView(this);
        mark.setGravity(Gravity.CENTER);
        mark.setTextSize(13);
        mark.setText(done ? "✓" : "!");
        mark.setTextColor(done ? color(R.color.on_accent) : 0xFFFFFFFF);
        GradientDrawable disc = new GradientDrawable();
        disc.setShape(GradientDrawable.OVAL);
        disc.setColor(color(done ? R.color.forest : R.color.attention));
        mark.setBackground(disc);
        mark.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        row.addView(mark, new LinearLayout.LayoutParams(pixels(24), pixels(24)));

        TextView text = text(label, 16, R.color.ink);
        LinearLayout.LayoutParams textParams =
            new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1);
        textParams.setMarginStart(pixels(12));
        row.addView(text, textParams);
        text.setContentDescription(label + (done ? "，已完成" : "，未完成"));

        if (!done) {
            TextView button = text(action, 15, R.color.forest);
            button.setGravity(Gravity.CENTER);
            button.setPadding(pixels(8), 0, pixels(8), 0);
            android.util.TypedValue ripple = new android.util.TypedValue();
            getTheme().resolveAttribute(android.R.attr.selectableItemBackground, ripple, true);
            button.setBackgroundResource(ripple.resourceId);
            button.setOnClickListener(ignored -> fix.run());
            row.addView(button, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, pixels(40)));
        }
        card.addView(row);
    }

    /** A scheme card: label and supporting line, a radio disc on the right, a 2dp accent ring when chosen. */
    private void schemeCard(LinearLayout column, SchemeCard option, boolean selected, int top) {
        boolean usable = snapshot != null && !saving;
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.HORIZONTAL);
        card.setGravity(Gravity.CENTER_VERTICAL);
        card.setPadding(pixels(16), pixels(14), pixels(16), pixels(14));
        GradientDrawable face = rounded(color(R.color.surface), pixels(20));
        if (selected) face.setStroke(pixels(2), color(R.color.forest));
        card.setBackground(face);

        LinearLayout text = new LinearLayout(this);
        text.setOrientation(LinearLayout.VERTICAL);
        TextView heading = text(option.label(), 16, R.color.ink);
        heading.setTypeface(Typeface.create(Typeface.DEFAULT, 600, false));
        text.addView(heading);
        TextView detail = text(option.detail(), 13, R.color.text_secondary);
        LinearLayout.LayoutParams detailParams = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        detailParams.topMargin = pixels(2);
        text.addView(detail, detailParams);
        card.addView(text, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));

        View radio = new View(this);
        GradientDrawable dot = new GradientDrawable();
        dot.setShape(GradientDrawable.OVAL);
        if (selected) {
            dot.setColor(color(R.color.mist));
            dot.setStroke(pixels(6), color(R.color.forest));
        } else {
            dot.setColor(0);
            dot.setStroke(Math.max(1, Math.round(1.5f * getResources().getDisplayMetrics().density)),
                color(R.color.text_secondary));
        }
        radio.setBackground(dot);
        LinearLayout.LayoutParams radioParams = new LinearLayout.LayoutParams(pixels(22), pixels(22));
        radioParams.setMarginStart(pixels(12));
        card.addView(radio, radioParams);

        card.setContentDescription(option.label() + "，" + option.detail()
            + (selected ? "，已选择" : "，未选择"));
        card.setEnabled(usable);
        card.setAlpha(usable || selected ? 1f : 0.6f);
        card.setOnClickListener(usable && !selected ? ignored -> selectScheme(option.scheme()) : null);
        card.setClickable(usable);
        column.addView(card, blockParams(top));
    }

    private void perk(LinearLayout column, @DrawableRes int icon, String label, int top) {
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setPadding(pixels(14), pixels(12), pixels(14), pixels(12));
        row.setBackground(rounded(color(R.color.surface), pixels(20)));
        ImageView badge = new ImageView(this);
        badge.setImageResource(icon);
        badge.setImageTintList(ColorStateList.valueOf(color(R.color.forest)));
        badge.setPadding(pixels(7), pixels(7), pixels(7), pixels(7));
        badge.setBackground(rounded(color(R.color.surface_variant), pixels(9)));
        badge.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        row.addView(badge, new LinearLayout.LayoutParams(pixels(32), pixels(32)));
        TextView text = text(label, 15, R.color.ink);
        LinearLayout.LayoutParams textParams =
            new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1);
        textParams.setMarginStart(pixels(12));
        row.addView(text, textParams);
        column.addView(row, blockParams(14 + top - 10));
    }

    private void footnote(LinearLayout column, String message) {
        TextView view = text(message, 13, R.color.text_secondary);
        view.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        column.addView(view, blockParams(14));
    }

    private TextView text(String value, int size, int colour) {
        TextView view = new TextView(this);
        view.setText(value);
        view.setTextSize(size);
        view.setTextColor(color(colour));
        return view;
    }

    private LinearLayout.LayoutParams blockParams(int top) {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.topMargin = pixels(top);
        return params;
    }

    private int color(int id) {
        return ContextCompat.getColor(this, id);
    }

    private static GradientDrawable rounded(int colour, int radius) {
        GradientDrawable shape = new GradientDrawable();
        shape.setCornerRadius(radius);
        shape.setColor(colour);
        return shape;
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

    private int pixels(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }
}
