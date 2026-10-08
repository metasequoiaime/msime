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
import android.view.animation.LinearInterpolator;
import android.view.animation.PathInterpolator;
import android.widget.ImageView;
import android.widget.ProgressBar;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONObject;
import androidx.fragment.app.FragmentTransaction;
import app.msime.android.AccountIdentity;
import app.msime.android.CommunityRequest;
import app.msime.android.FirstRunPreparation;
import app.msime.android.HostDeepLink;
import app.msime.android.core.Telemetry;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import com.google.android.material.bottomnavigation.BottomNavigationView;

/**
 * 宿主主界面：底部四个 tab 共用一个 Fragment 容器，详情页压在同一个容器里。
 *
 * First-run dictionary preparation is started from here, because this is the launcher and there is
 * nowhere else the user reliably arrives. It never enables or selects the input method on their
 * behalf -- that remains a decision taken in system settings.
 *
 * Each tab is created once and then hidden rather than replaced. Replacing tore the page down on
 * every switch: coming back to 社区 re-fetched the listing over the network and threw away how far
 * the user had scrolled, and 统计 forgot which of its four segments was open.
 *
 * <p>导航只有一个回退栈：{@link SettingsNavigator} 把详情页压进 `home_content`（底部导航保持可见），返回一层弹出一页，栈空了再回到第一个 tab，再返回才离开应用。切到别的 tab 或再点一次当前 tab 都先清空栈，所以返回不会把用户带回另一个 tab 里的旧页面。进程被杀后 FragmentManager 自己恢复 tab 页和栈，这里不会再添加一遍。
 *
 * <p>本 Activity 是 exported 的：{@link HostDeepLink} 形式的 Intent 在 `onCreate`（首次创建时）和 `onNewIntent` 里解析，页面只按 `PageId` 的枚举名映射，不认识的忽略，参数只影响导航。
 */
public final class HomeActivity extends AppCompatActivity {
    private static final String STATE_TAB = "home-tab";
    /** Set while a first-launch splash is on screen, so that a rotation mid-splash still hands over to onboarding. */
    private static final String STATE_ONBOARDING_AFTER_INTRO = "home-onboarding-after-intro";
    private static final int FIRST_TAB = R.id.tab_settings;
    private static final String STORE = "msime_home_v1";
    private static final String SPLASH_SEEN = "splash_seen";
    /** 开屏停留的总时长：进度条 2.4 s 走满，再停一口气，2.8 s 交接。 */
    private static final long INTRO_HOLD_MILLIS = 2800;
    /** 底部进度条走满的时长。 */
    private static final long INTRO_PROGRESS_MILLIS = 2400;
    private static final long INTRO_FADE_MILLIS = 260;
    /** The design's msPop curve, cubic-bezier(.16, 1, .3, 1). */
    private static final PathInterpolator POP = new PathInterpolator(0.16f, 1f, 0.3f, 1f);
    private static final PathInterpolator EASE = new PathInterpolator(0.25f, 0.1f, 0.25f, 1f);
    private static final int[] TAB_IDS = {
        R.id.tab_settings, R.id.tab_community, R.id.tab_statistics, R.id.tab_account,
    };

    private BottomNavigationView tabs;
    private OnBackPressedCallback back;
    /** 保存状态之后才到达的深链，等回到前台再执行。 */
    @Nullable private Intent pendingLink;
    private int selected = FIRST_TAB;
    @Nullable private CommunityRequest.Kind pendingKind;
    /** A tab whose kept instance is stale and has to be built again on the next switch to it. */
    private int rebuild;
    @Nullable private ValueAnimator breath;
    /** Whether dismissing the splash on screen should start onboarding: only a first-launch splash does, never a replay. */
    private boolean onboardingAfterIntro;
    @Nullable private ValueAnimator progress;
    /** 这个 activity 画出来时叠的季节（`AppMode.restore` 用的那份缓存）；没有缓存时是基础主题的秋杉。 */
    private String drawnSeason = "autumn";
    /** 回到前台时读共享偏好、解析应用主题用的工作线程。 */
    private final ExecutorService themeWorker = Executors.newSingleThreadExecutor();

    @Override protected void onCreate(Bundle state) {
        AppMode.restore(this);
        String cached = AppThemeController.cachedSeason(this);
        if (cached != null) drawnSeason = cached;
        super.onCreate(state);
        Telemetry.start(this);
        AccountIdentity.register(this);
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
            ViewPolicy.setPadding(content, bars.left, bars.top, bars.right, 0);
            ViewPolicy.setPadding(tabs, bars.left, 0, bars.right, bars.bottom);
            ViewPolicy.setPadding(intro, 0, 0, 0, bars.bottom);
            return windowInsets;
        });
        ViewPolicy.bindClick(intro, this::dismissIntro);

        // 返回先一层层弹出详情页，栈空了回到第一个 tab，最后才离开应用，这是底部导航让用户预期的顺序。
        // 只用这一个回调：它在 FragmentManager 自己的回调之后注册、优先级更高，两个都处理返回会在有栈时把 tab 也切走。
        // 在第一个 tab 的根页时回调关闭，交给系统默认处理，离开应用仍有平台自己的返回动画，而不是光秃秃的 finish()。
        back = new OnBackPressedCallback(false) {
            @Override public void handleOnBackPressed() {
                FragmentManager manager = getSupportFragmentManager();
                if (manager.getBackStackEntryCount() > 0) manager.popBackStack();
                else tabs.setSelectedItemId(FIRST_TAB);
            }
        };
        getOnBackPressedDispatcher().addCallback(this, back);
        getSupportFragmentManager().addOnBackStackChangedListener(this::updateBack);

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
        // 先在没有监听器时标出选中项，再装监听器：否则这一下会被当成「再点一次当前 tab」而清掉恢复回来的回退栈。
        tabs.setSelectedItemId(selected);
        tabs.setOnItemSelectedListener(item -> {
            selectTab(item.getItemId());
            return true;
        });
        // 再点一次当前 tab：回到这个 tab 的根页。
        tabs.setOnItemReselectedListener(item -> clearStack());
        // 进程恢复时 FragmentManager 已经把 tab 页和回退栈原样还原（包括谁被藏起来），有栈时再 show 会把栈底的 tab 页叠到详情页上面。
        if (getSupportFragmentManager().getBackStackEntryCount() == 0) show(selected);
        updateBack();

        // The shipped dictionary is prepared on first run without the user having to find a button
        // for it: a keyboard that cannot reach the Engine is not a state worth making someone opt
        // out of. Existing configurations are reported, never overwritten.
        // 先挂上引导页记下的待保存选择：上次在准备完成前选了方案就离开、或进程被杀，这次准备完成时把它写进去。
        OnboardingChoices.watch(this);
        FirstRunPreparation.startIfNeeded(this);

        // 只有首次创建才执行 Intent 里的深链：旋转、换深浅模式和进程恢复时 getIntent() 还是那一个，再执行一遍会把用户已经离开的页面又压回来。
        if (state == null) openDeepLink(getIntent());
    }

    @Override protected void onNewIntent(@NonNull Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        openDeepLink(intent);
    }

    /** 每次回到前台都按今天的月份重新解析一次应用主题：「水杉四季」跨季节时，开着的页面要换成新季节的颜色。 */
    @Override protected void onResume() {
        super.onResume();
        themeWorker.execute(() -> {
            JSONObject snapshot = HostStore.prepared(this) ? HostStore.loadPreferences(this) : null;
            JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
            if (preferences == null) return;
            AppThemeController.follow(this, preferences);
            runOnUiThread(this::recreateIfSeasonChanged);
        });
        CloudSync.onResume(this);
    }

    /**
     * 缓存的季节和画出来的不一样时重建。设置首页在工作线程里按刚读到的偏好 {@link AppThemeController#follow} 之后也调用它，所以在应用里改「应用主题」或换季都会立即生效。只比较 SharedPreferences 里的缓存，不读文件、不调 Rust。
     */
    void recreateIfSeasonChanged() {
        if (isFinishing() || isDestroyed()) return;
        String season = AppThemeController.cachedSeason(this);
        if (season == null || season.equals(drawnSeason)) return;
        // 开屏正在放时不打断它：放完之后下一次回到前台再换。
        View intro = findViewById(R.id.home_intro);
        if (intro != null && intro.getVisibility() == View.VISIBLE) return;
        drawnSeason = season;
        recreate();
    }

    @Override protected void onPostResume() {
        super.onPostResume();
        Intent link = pendingLink;
        pendingLink = null;
        if (link != null) openDeepLink(link);
    }

    /**
     * 执行一个 {@link HostDeepLink}：先回到目标 tab 的根页，再按需压入页面。
     *
     * <p>页面名只经 {@link PageId#fromName} 映射，不认识就当作只给了 tab；参数原样交给页面，并带着 `HostDeepLink.ARG_EXTERNAL`，页面据此只把它用于导航和滚动。
     */
    private void openDeepLink(@Nullable Intent intent) {
        HostDeepLink.Request request = HostDeepLink.read(intent);
        if (request.isEmpty()) return;
        if (getSupportFragmentManager().isStateSaved()) {
            pendingLink = intent;
            return;
        }
        PageId page = PageId.fromName(request.page);
        int tab = page != null ? page.tab() : request.tab;
        if (tab == HostDeepLink.NO_TAB) return;
        selectTabIndex(tab);
        clearStack();
        if (page != null) SettingsNavigator.push(this, page, request.args);
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
     * 开场：本季底色上光晕浮起，浅色圆盘弹入，那枚标随后旋入并自己写一遍，名字与拉丁名升上来，底部进度条 2.4 秒走满，停满 2.8 秒或被轻点后让开。
     *
     * <p>Drawn by the app rather than by the platform's splash screen. Timing follows the design's keyframes: msCircIn for the disc (0.65 s), msLogoIn for the mark (0.6 s from 0.25 s, turning in from -30°), the stroke drawn by the animated vector, msRipple on the ring twice from 1.45 s, the name and the Latin line fading up at 0.9 and 1.1 s, and msBreath on the halo from 1.4 s.
     */
    private void playIntro(boolean leadsToOnboarding) {
        View intro = findViewById(R.id.home_intro);
        onboardingAfterIntro = leadsToOnboarding;
        intro.animate().cancel();
        intro.setAlpha(1f);
        ViewPolicy.setClickable(intro, true);
        ViewPolicy.show(intro);
        barsOnDark(true);

        View glow = findViewById(R.id.home_intro_glow);
        stopBreath();
        pop(glow, 800, 0, EASE);
        View disc = findViewById(R.id.home_intro_disc);
        pop(disc, 650, 0, POP);
        ImageView mark = findViewById(R.id.home_intro_mark);
        mark.animate().cancel();
        mark.setAlpha(0f);
        mark.setScaleX(0.6f);
        mark.setScaleY(0.6f);
        mark.setRotation(-30f);
        mark.animate().alpha(1f).scaleX(1f).scaleY(1f).rotation(0f)
            .setDuration(600).setStartDelay(250).setInterpolator(POP).start();
        if (mark.getDrawable() instanceof Animatable animatable) {
            animatable.stop();
            animatable.start();
        }
        ripple(findViewById(R.id.home_intro_ring));
        fadeUp(findViewById(R.id.home_intro_name), 900);
        fadeUp(findViewById(R.id.home_intro_latin), 1100);

        ProgressBar bar = findViewById(R.id.home_intro_progress);
        if (progress != null) progress.cancel();
        bar.setProgress(0);
        progress = ValueAnimator.ofInt(0, bar.getMax());
        progress.setDuration(INTRO_PROGRESS_MILLIS);
        progress.setInterpolator(new LinearInterpolator());
        progress.addUpdateListener(animation -> bar.setProgress((int) animation.getAnimatedValue()));
        progress.start();

        // One half-cycle of msBreath is 1.2 s, run back and forth until the splash leaves. It starts from where the pop ends (fully lit, full size) and dims while it swells, so there is no jump at 1.4 s.
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

    /** 设计的 msRipple：圆盘外的圆环从圆盘大小放到 1.2 倍并淡出，1.45 s 起放两次，每次 1.2 s。 */
    private static void ripple(View ring) {
        ring.animate().cancel();
        ring.setAlpha(0f);
        ring.setScaleX(1f);
        ring.setScaleY(1f);
        ring.animate().setStartDelay(1450).setDuration(0).withEndAction(() -> rippleOnce(ring, 2)).start();
    }

    private static void rippleOnce(View ring, int remaining) {
        if (remaining <= 0) return;
        ring.setAlpha(0.5f);
        ring.setScaleX(1f);
        ring.setScaleY(1f);
        ring.animate().alpha(0f).scaleX(1.2f).scaleY(1.2f).setStartDelay(0).setDuration(1200)
            .setInterpolator(EASE).withEndAction(() -> rippleOnce(ring, remaining - 1)).start();
    }

    private final Runnable dismissIntro = this::dismissIntro;

    /** Fade the splash out, whether its time ran out or it was tapped, and hand over to onboarding if this was the first launch. */
    private void dismissIntro() {
        View intro = findViewById(R.id.home_intro);
        if (intro.getVisibility() != View.VISIBLE || !intro.isClickable()) return;
        intro.removeCallbacks(dismissIntro);
        // Not clickable while it fades, so a second tap during the fade does not start onboarding twice.
        ViewPolicy.setClickable(intro, false);
        boolean onboarding = onboardingAfterIntro;
        onboardingAfterIntro = false;
        intro.animate().alpha(0f).setDuration(INTRO_FADE_MILLIS).withEndAction(() -> {
            ViewPolicy.hide(intro);
            ViewPolicy.setClickable(intro, true);
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
        if (progress != null) progress.cancel();
        progress = null;
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
        view.setTranslationY(Ui.dp(this, 10));
        view.animate().alpha(1f).translationY(0f)
            .setDuration(500).setStartDelay(delay).setInterpolator(EASE).start();
    }

    @Override protected void onDestroy() {
        stopBreath();
        themeWorker.shutdownNow();
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

    /** 当前 tab 的下标，取值是 `HostDeepLink.TAB_*`。 */
    int selectedTabIndex() {
        for (int i = 0; i < TAB_IDS.length; i++) {
            if (TAB_IDS[i] == selected) return i;
        }
        return HostDeepLink.TAB_SETTINGS;
    }

    /** 按下标切 tab（`HostDeepLink.TAB_*`）；切换时清空回退栈。 */
    void selectTabIndex(int index) {
        if (!HostDeepLink.isTab(index)) return;
        openTab(TAB_IDS[index]);
    }

    /** 底部导航的选中项变了：换 tab 之前先把详情页全部弹出。 */
    private void selectTab(int itemId) {
        if (itemId != selected) clearStack();
        show(itemId);
    }

    /** 弹出所有详情页，回到当前 tab 的根页。 */
    private void clearStack() {
        FragmentManager manager = getSupportFragmentManager();
        if (manager.getBackStackEntryCount() == 0 || manager.isStateSaved()) return;
        manager.popBackStackImmediate(manager.getBackStackEntryAt(0).getId(),
            FragmentManager.POP_BACK_STACK_INCLUSIVE);
    }

    /** 返回键由我们处理的条件：有详情页可弹，或者不在第一个 tab。 */
    private void updateBack() {
        if (back == null) return;
        // back 是 OnBackPressedCallback，不是 View，ViewPolicy.setEnabled 管不了它。
        back.setEnabled(getSupportFragmentManager().getBackStackEntryCount() > 0 || selected != FIRST_TAB);
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
        clearStack();
        if (selected == R.id.tab_community) show(R.id.tab_community);
        else openTab(R.id.tab_community);
    }

    private void show(int itemId) {
        boolean switching = selected != itemId;
        selected = itemId;
        updateBack();
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
