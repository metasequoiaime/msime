package app.msime.android.home;

import android.animation.ValueAnimator;
import android.content.Intent;
import android.os.Bundle;
import androidx.activity.OnBackPressedCallback;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;
import androidx.fragment.app.Fragment;
import androidx.fragment.app.FragmentManager;
import android.graphics.drawable.Animatable;
import android.view.View;
import android.view.animation.PathInterpolator;
import android.widget.ImageView;
import androidx.fragment.app.FragmentTransaction;
import app.msime.android.CommunityRequest;
import app.msime.android.FirstRunPreparation;
import app.msime.android.core.Telemetry;
import app.msime.android.R;
import com.google.android.material.bottomnavigation.BottomNavigationView;

/**
 * The host app: four tabs over one fragment container, matching the Apple app's shell.
 *
 * First-run dictionary preparation is started from here, because this is the launcher and there is
 * nowhere else the user reliably arrives. It never enables or selects the input method on their
 * behalf -- that remains a decision taken in system settings.
 *
 * Each tab is created once and then hidden rather than replaced. Replacing tore the page down on
 * every switch: coming back to 社区 re-fetched the listing over the network and threw away how far
 * the user had scrolled, and 统计 forgot which of its four segments was open.
 */
public final class HomeActivity extends AppCompatActivity {
    private static final String STATE_TAB = "home-tab";
    /** Set while a first-launch splash is on screen, so that a rotation mid-splash still hands over to onboarding. */
    private static final String STATE_ONBOARDING_AFTER_INTRO = "home-onboarding-after-intro";
    private static final int FIRST_TAB = R.id.tab_settings;
    private static final String STORE = "msime_home_v1";
    private static final String SPLASH_SEEN = "splash_seen";
    /** 开屏停留的总时长：描边在 1.45 s 写完，名字和「轻点跳过」在 2.1 s 前都已站定，再留一口气。 */
    private static final long INTRO_HOLD_MILLIS = 2800;
    private static final long INTRO_FADE_MILLIS = 260;
    /** The design's msPop curve, cubic-bezier(.16, 1, .3, 1). */
    private static final PathInterpolator POP = new PathInterpolator(0.16f, 1f, 0.3f, 1f);
    private static final PathInterpolator EASE = new PathInterpolator(0.25f, 0.1f, 0.25f, 1f);
    private static final int[] TAB_IDS = {
        R.id.tab_settings, R.id.tab_community, R.id.tab_statistics, R.id.tab_account,
    };

    private BottomNavigationView tabs;
    private OnBackPressedCallback back;
    private int selected = FIRST_TAB;
    @Nullable private CommunityRequest.Kind pendingKind;
    /** A tab whose kept instance is stale and has to be built again on the next switch to it. */
    private int rebuild;
    @Nullable private ValueAnimator breath;
    /** Whether dismissing the splash on screen should start onboarding: only a first-launch splash does, never a replay. */
    private boolean onboardingAfterIntro;

    @Override protected void onCreate(Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        Telemetry.start(this);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        setContentView(R.layout.activity_home);

        tabs = findViewById(R.id.home_tabs);
        // Edge-to-edge is enforced from Android 15, so the bars' insets are applied rather than
        // assumed: the tab bar keeps clear of the gesture area and the content of the status bar.
        // The splash overlay is the one full-bleed child, so the insets go on the content and the tab bar rather than on the root: padding the root would leave a strip of page colour above the splash.
        View content = findViewById(R.id.home_content);
        View intro = findViewById(R.id.home_intro);
        ViewCompat.setOnApplyWindowInsetsListener(findViewById(R.id.home_root), (view, windowInsets) -> {
            Insets bars = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars());
            content.setPadding(bars.left, bars.top, bars.right, 0);
            tabs.setPadding(bars.left, 0, bars.right, bars.bottom);
            intro.setPadding(0, 0, 0, bars.bottom);
            return windowInsets;
        });
        intro.setOnClickListener(ignored -> dismissIntro());

        // Back returns to the first tab before it leaves the app, which is what a bottom bar leads
        // the user to expect. On the first tab the callback is off and the system default runs, so
        // leaving still gets the platform's own back animation rather than a bare finish().
        back = new OnBackPressedCallback(false) {
            @Override public void handleOnBackPressed() { tabs.setSelectedItemId(FIRST_TAB); }
        };
        getOnBackPressedDispatcher().addCallback(this, back);

        // 第一次打开：先放开屏，放完（或被轻点跳过）再走引导——这台设备还没见过它，而它讲的正是「键盘怎么用起来」。
        // A rotation is not an arrival and plays nothing; if it lands mid-splash, the handover to onboarding that the splash owed is paid here instead.
        if (state == null) {
            if (firstLaunch()) {
                markSplashSeen();
                playIntro(true);
            } else if (!OnboardingActivity.seen(this)) {
                startActivity(new Intent(this, OnboardingActivity.class));
            }
        } else if (state.getBoolean(STATE_ONBOARDING_AFTER_INTRO) && !OnboardingActivity.seen(this)) {
            startActivity(new Intent(this, OnboardingActivity.class));
        }

        if (state != null) selected = state.getInt(STATE_TAB, FIRST_TAB);
        tabs.setOnItemSelectedListener(item -> {
            show(item.getItemId());
            return true;
        });
        show(selected);
        tabs.setSelectedItemId(selected);

        // The shipped dictionary is prepared on first run without the user having to find a button
        // for it: a keyboard that cannot reach the Engine is not a state worth making someone opt
        // out of. Existing configurations are reported, never overwritten.
        FirstRunPreparation.startIfNeeded(this);
    }

    /**
     * 开屏只在这台设备第一次打开时放：之后每次启动都放一遍，2.8 秒就成了每次打开都要等的时间。
     *
     * <p>A device that already finished onboarding is not on its first launch either, which keeps an update from greeting existing users with a splash they never had.
     */
    private boolean firstLaunch() {
        return !getSharedPreferences(STORE, MODE_PRIVATE).getBoolean(SPLASH_SEEN, false)
            && !OnboardingActivity.seen(this);
    }

    /** Marked when the splash starts rather than when it ends, so a launch killed mid-splash does not play it again. */
    private void markSplashSeen() {
        getSharedPreferences(STORE, MODE_PRIVATE).edit().putBoolean(SPLASH_SEEN, true).apply();
    }

    /**
     * Play the splash again, as 我的 → 开屏动画 asks. A replay is only a replay: dismissing it returns to the page it covered and never opens onboarding, whether or not onboarding was ever finished. (The prototype sends a tap on any splash into onboarding; that is the defect this avoids.)
     */
    public void replaySplash() {
        playIntro(false);
    }

    /**
     * 开场：光晕浮起，那枚标自己写一遍，名字随后升上来，停满 2.8 秒或被轻点后让开。
     *
     * <p>Drawn by the app rather than by the platform's splash screen. The theme attributes for it were configured and on this device nothing used them — a splash background set to pure red never appeared in a hundred recorded frames. Timing follows the design's keyframes: msPop for the halo (0.8 s) and the mark (0.7 s), msDraw for the stroke (1.05 s from 0.4 s, in the animated vector), msFadeUp for the three lines of text at 1.1, 1.35 and 1.6 s, and msBreath on the halo from 1.4 s.
     */
    private void playIntro(boolean leadsToOnboarding) {
        View intro = findViewById(R.id.home_intro);
        onboardingAfterIntro = leadsToOnboarding;
        intro.animate().cancel();
        intro.setAlpha(1f);
        intro.setClickable(true);
        intro.setVisibility(View.VISIBLE);
        barsOnDark(true);

        View glow = findViewById(R.id.home_intro_glow);
        stopBreath();
        pop(glow, 800, 0, EASE);
        ImageView mark = findViewById(R.id.home_intro_mark);
        pop(mark, 700, 0, POP);
        if (mark.getDrawable() instanceof Animatable animatable) {
            animatable.stop();
            animatable.start();
        }
        fadeUp(findViewById(R.id.home_intro_name), 1100);
        fadeUp(findViewById(R.id.home_intro_latin), 1350);
        fadeUp(findViewById(R.id.home_intro_skip), 1600);

        // One half-cycle of msBreath is 1.2 s, run back and forth until the splash leaves. It starts from where the pop ends (fully lit, full size) and dims while it swells, so there is no jump at 1.4 s; the prototype's keyframes restart at 55 % opacity there.
        breath = ValueAnimator.ofFloat(0f, 1f);
        breath.setDuration(1200);
        breath.setStartDelay(1400);
        breath.setRepeatCount(ValueAnimator.INFINITE);
        breath.setRepeatMode(ValueAnimator.REVERSE);
        breath.setInterpolator(EASE);
        breath.addUpdateListener(animation -> {
            float t = (float) animation.getAnimatedValue();
            glow.setAlpha(1f - 0.45f * t);
            glow.setScaleX(1f + 0.04f * t);
            glow.setScaleY(1f + 0.04f * t);
        });
        breath.start();

        intro.removeCallbacks(dismissIntro);
        intro.postDelayed(dismissIntro, INTRO_HOLD_MILLIS);
    }

    private final Runnable dismissIntro = this::dismissIntro;

    /** Fade the splash out, whether its time ran out or it was tapped, and hand over to onboarding if this was the first launch. */
    private void dismissIntro() {
        View intro = findViewById(R.id.home_intro);
        if (intro.getVisibility() != View.VISIBLE || !intro.isClickable()) return;
        intro.removeCallbacks(dismissIntro);
        // Not clickable while it fades, so a second tap during the fade does not start onboarding twice.
        intro.setClickable(false);
        boolean onboarding = onboardingAfterIntro;
        onboardingAfterIntro = false;
        intro.animate().alpha(0f).setDuration(INTRO_FADE_MILLIS).withEndAction(() -> {
            intro.setVisibility(View.GONE);
            intro.setClickable(true);
            stopBreath();
        }).start();
        barsOnDark(false);
        if (onboarding && !OnboardingActivity.seen(this)) {
            startActivity(new Intent(this, OnboardingActivity.class));
        }
    }

    private void stopBreath() {
        if (breath != null) breath.cancel();
        breath = null;
    }

    /** Light status and navigation bar icons over the splash's dark field; the theme's own choice for the day or night page afterwards. */
    private void barsOnDark(boolean dark) {
        WindowInsetsControllerCompat controller =
            WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        boolean light = !dark && getResources().getBoolean(R.bool.light_bars);
        controller.setAppearanceLightStatusBars(light);
        controller.setAppearanceLightNavigationBars(light);
    }

    private static void pop(View view, long duration, long delay, PathInterpolator curve) {
        view.animate().cancel();
        view.setAlpha(0f);
        view.setScaleX(0.86f);
        view.setScaleY(0.86f);
        view.animate().alpha(1f).scaleX(1f).scaleY(1f)
            .setDuration(duration).setStartDelay(delay).setInterpolator(curve).start();
    }

    private void fadeUp(View view, long delay) {
        view.animate().cancel();
        view.setAlpha(0f);
        view.setTranslationY(10 * getResources().getDisplayMetrics().density);
        view.animate().alpha(1f).translationY(0f)
            .setDuration(500).setStartDelay(delay).setInterpolator(EASE).start();
    }

    @Override protected void onDestroy() {
        stopBreath();
        super.onDestroy();
    }

    @Override protected void onSaveInstanceState(@NonNull Bundle state) {
        super.onSaveInstanceState(state);
        state.putInt(STATE_TAB, selected);
        state.putBoolean(STATE_ONBOARDING_AFTER_INTRO, onboardingAfterIntro);
    }

    /** Switch to one of the four tabs, as a row on another tab can ask to. */
    public void openTab(int itemId) {
        if (tabs != null) tabs.setSelectedItemId(itemId);
    }

    /**
     * Switch to the community tab and open it on one kind of work.
     *
     * <p>The kept instance is discarded for this: which kind the tab opens on is an argument, and
     * the one on screen is showing another.
     */
    public void openCommunity(CommunityRequest.Kind kind) {
        pendingKind = kind;
        rebuild = R.id.tab_community;
        if (selected == R.id.tab_community) show(R.id.tab_community);
        else openTab(R.id.tab_community);
    }

    private void show(int itemId) {
        boolean switching = selected != itemId;
        selected = itemId;
        back.setEnabled(itemId != FIRST_TAB);
        FragmentManager manager = getSupportFragmentManager();
        FragmentTransaction transaction = manager.beginTransaction();
        // Fade-through between tabs: the page arriving fades in from 94 % scale while the one leaving is hidden at once, as the M3 navigation bar pattern prescribes. The first show on create and a rebuild of the tab already on screen are not arrivals, so they appear without it.
        if (switching) transaction.setCustomAnimations(R.anim.fade_through_in, 0);
        for (int id : TAB_IDS) {
            Fragment page = manager.findFragmentByTag(tag(id));
            if (id != itemId) {
                if (page != null && !page.isHidden()) transaction.hide(page);
                continue;
            }
            // Removing and adding in one transaction rather than committing the removal on its own:
            // a synchronous commit here can land on top of a tab switch that has not run yet.
            if (page != null && rebuild == id) {
                transaction.remove(page);
                page = null;
            }
            if (page == null) transaction.add(R.id.home_content, create(id), tag(id));
            else transaction.show(page);
        }
        rebuild = 0;
        // 同步提交：`commit` 是排队执行的，而这个方法靠 findFragmentByTag 判断某个 tab 建过没有。
        // 两次调用挨在一起时——onCreate 里先 show 再 setSelectedItemId 触发一次，或者快速连点
        // 两个 tab——第二次查不到第一次还没执行的 add，于是同一个 tag 被加了两遍，两个页面一起画，
        // 看起来就是 UI 叠在一起。保存状态之后不能同步提交，那种时候退回排队，晚一点总比崩了好。
        if (manager.isStateSaved()) transaction.commit(); else transaction.commitNow();
    }

    private Fragment create(int itemId) {
        if (itemId == R.id.tab_community) {
            CommunityRequest.Kind kind = pendingKind;
            pendingKind = null;
            return CommunityFragment.forKind(kind);
        }
        if (itemId == R.id.tab_statistics) return new StatisticsFragment();
        if (itemId == R.id.tab_account) return new AccountFragment();
        return new KeyboardFragment();
    }

    private static String tag(int itemId) { return "home-tab-" + itemId; }
}
