package app.msime.android.home;

import app.msime.android.ScreenCaptureProtection;
import app.msime.android.DimensionPolicy;

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
import app.msime.android.AppEdition;
import app.msime.android.Bootstrap;
import app.msime.android.CommunityRequest;
import app.msime.android.FirstRunPreparation;
import app.msime.android.HostDeepLink;
import app.msime.android.KeyboardScheme;
import app.msime.android.ResourcePackService;
import app.msime.android.ResourcePacks;
import app.msime.android.SchemePreferences;
import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.policy.HostOptionsPolicy;
import app.msime.android.core.Telemetry;
import app.msime.android.R;
import app.msime.android.ThreadPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.WindowInsetsPolicy;
import com.google.android.material.bottomnavigation.BottomNavigationView;

/**
 * 宿主主界面：底部四个 tab（设置、社区、统计、我的）共用一个 Fragment 容器，详情页压在同一个容器里。第二格是社区：根页是 {@link CommunityTabPage}（皮肤 / 词库两个分类，各自是完整的社区列表），以 {@link DetailPage#asTabRoot} 承载、没有返回按钮；各处打开社区皮肤或社区词库完整列表的入口都切到这个 tab 并选中对应分类（见 {@link SettingsNavigator}）。AI 对话通过设置导航栈打开二级页面。
 *
 * First-run dictionary preparation is started from here, because this is the launcher and there is
 * nowhere else the user reliably arrives. It never enables or selects the input method on their
 * behalf -- that remains a decision taken in system settings.
 *
 * 每个 tab 只建一次，之后藏起来而不是替换掉。替换会在每次切换时把页面拆掉：统计会忘了打开的是四段里的哪一段，滚动位置也跟着丢。
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
    /** 按 `HostDeepLink.TAB_*` 下标排的底部导航项，顺序与 `res/menu/bottom_tabs.xml` 相同（`tests/home/BottomTabsSmoke.java` 核对）。 */
    private static final int[] TAB_IDS = {
        R.id.tab_settings, R.id.tab_community, R.id.tab_statistics, R.id.tab_account,
    };

    private BottomNavigationView tabs;
    /** 当前窗口是否显示系统输入法；搜索框获得焦点时底部导航必须让出空间。 */
    private boolean imeVisible;
    private OnBackPressedCallback back;
    /** 保存状态之后才到达的深链，等回到前台再执行。 */
    @Nullable private Intent pendingLink;
    private int selected = FIRST_TAB;
    /** A tab whose kept instance is stale and has to be built again on the next switch to it. */
    private int rebuild;
    /** 换 tab 前清栈的那一次同步弹栈期间为 true：这时弹出的详情页和被弹回来的 tab 根页都马上要离开屏幕，不播回退动画（见 {@link HomeTabFragment#leavingWithoutAnimation}）。 */
    private boolean clearingForTabSwitch;
    @Nullable private ValueAnimator breath;
    /** Whether dismissing the splash on screen should start onboarding: only a first-launch splash does, never a replay. */
    private boolean onboardingAfterIntro;
    @Nullable private ValueAnimator progress;
    @Nullable private Runnable hideDetailLoading;
    /** 社区 tab 还没建时要选中的分类，建它的时候交给根页。 */
    @Nullable private CommunityRequest.Kind communityKind;
    /** 这个 activity 画出来时叠的季节（`AppMode.restore` 用的那份缓存）；没有缓存时是基础主题的秋杉。 */
    private String drawnSeason = "autumn";
    /** 回到前台时读共享偏好、解析应用主题用的工作线程。 */
    private final ExecutorService themeWorker = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-home-theme"));
    /** 这个进程里是否已经替用户请求过五笔码表（{@link #requestWubiTableIfWanted}）：用户在计费网络的确认框里点了取消、或当时没联网，到下次启动应用再问，不在每次回到前台时都打扰。 */
    private static final java.util.concurrent.atomic.AtomicBoolean WUBI_TABLE_REQUESTED =
        new java.util.concurrent.atomic.AtomicBoolean();
    /** 安装包的临时说明（{@link InstallNotice}）正显示着：五笔码表的下载询问等它关掉再弹。只在主线程上读写。 */
    private boolean installNoticeShowing;

    @Override protected void onCreate(Bundle state) {
        app.msime.android.LauncherIcons.normalize(this);
        AppMode.restore(this);
        String cached = AppThemeController.cachedSeason(this);
        if (cached != null) drawnSeason = cached;
        super.onCreate(state);
        // 「防止截屏」（#10098）：窗口显示前按本地设置加上 FLAG_SECURE。
        ScreenCaptureProtection.apply(this);
        Telemetry.start(this);
        AccountIdentity.register(this);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        setContentView(R.layout.activity_home);

        getSupportFragmentManager().registerFragmentLifecycleCallbacks(
            new FragmentManager.FragmentLifecycleCallbacks() {
                @Override public void onFragmentViewCreated(@NonNull FragmentManager manager,
                        @NonNull Fragment fragment, @NonNull View view, @Nullable Bundle state) {
                    // 根 Tab 和详情页都在 home_content 中；不能只判断 DetailPage，否则首次打开设置/社区/统计/我的时全局 loading 会没有对应的关闭回调。
                    if (fragment.getId() == R.id.home_content) setDetailLoading(false);
                }
            }, true);

        tabs = findViewById(R.id.home_tabs);
        // Edge-to-edge is enforced from Android 15, so the bars' insets are applied rather than
        // assumed: the tab bar keeps clear of the gesture area and the content of the status bar.
        // The splash overlay is the one full-bleed child, so the insets go on the content and the tab bar rather than on the root: padding the root would leave a strip of page colour above the splash.
        View content = findViewById(R.id.home_content);
        View intro = findViewById(R.id.home_intro);
        ViewCompat.setOnApplyWindowInsetsListener(findViewById(R.id.home_root), (view, windowInsets) -> {
            Insets bars = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars());
            imeVisible = windowInsets.isVisible(WindowInsetsCompat.Type.ime());
            ViewPolicy.setPadding(content, bars.left, bars.top, bars.right, 0);
            ViewPolicy.setPadding(tabs, bars.left, 0, bars.right, bars.bottom);
            ViewPolicy.setPadding(intro, 0, 0, 0, bars.bottom);
            updateTabsVisibility();
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
        getSupportFragmentManager().addOnBackStackChangedListener(() -> {
            updateBack();
            updateTabsVisibility();
        });

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
        // 保存下来的 tab 不在现在的底部导航里时回到第一个 tab。
        if (tabIndex(selected) == HostDeepLink.NO_TAB) selected = FIRST_TAB;
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
        updateTabsVisibility();

        // The shipped dictionary is prepared on first run without the user having to find a button
        // for it: a keyboard that cannot reach the Engine is not a state worth making someone opt
        // out of. Existing configurations are reported, never overwritten.
        // 先挂上引导页记下的待保存选择：上次在准备完成前选了方案就离开、或进程被杀，这次准备完成时把它写进去。
        OnboardingChoices.watch(this);
        FirstRunPreparation.observe(dictionaryPreparation);
        FirstRunPreparation.startIfNeeded(this);

        // 只有首次创建才执行 Intent 里的深链：旋转、换深浅模式和进程恢复时 getIntent() 还是那一个，再执行一遍会把用户已经离开的页面又压回来。
        if (state == null) openDeepLink(getIntent());
    }

    @Override protected void onNewIntent(@NonNull Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        openDeepLink(intent);
    }

    // 首次解包完成也检查一次，避免首次 onResume 早于 runtime-options.json 写入。
    private final FirstRunPreparation.Listener dictionaryPreparation = state -> {
        if (state == FirstRunPreparation.State.READY && !themeWorker.isShutdown()) {
            themeWorker.execute(() -> DictionaryUpdates.request(getApplicationContext(), "automatic", null));
            requestWubiTableIfWanted();
        }
    };

    /** 每次回到前台都按今天的月份重新解析一次应用主题：「水杉四季」跨季节时，开着的页面要换成新季节的颜色。 */
    @Override protected void onResume() {
        super.onResume();
        // 设置应用常驻在最近任务里，另一个实例改了「防止截屏」后回到这里要跟上。
        ScreenCaptureProtection.apply(this);
        View intro = findViewById(R.id.home_intro);
        if (!ViewPolicy.isVisible(intro) && OnboardingActivity.seen(this)) {
            // 升级后第一次打开时说明和五笔码表的下载询问会同时到来，叠在一起时下面那个看不清：说明关掉之后再问。
            installNoticeShowing = InstallNotice.showIfNeeded(this, () -> {
                installNoticeShowing = false;
                requestWubiTableIfWanted();
            });
        }
        themeWorker.execute(() -> {
            JSONObject snapshot = HostStore.prepared(this) ? HostStore.loadPreferences(this) : null;
            JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
            if (preferences == null) return;
            AppThemeController.follow(this, preferences);
            runOnUiThread(this::recreateIfSeasonChanged);
        });
        CloudSync.onResume(this);
        // 打开了「上传到云端」时补传打字统计；十分钟的节流在共享层，没登录真实账号时什么都不发。
        TypingUpload.onResume(this);
        themeWorker.execute(() -> DictionaryUpdates.request(getApplicationContext(), "automatic", null));
        requestWubiTableIfWanted();
    }

    /**
     * 在用五笔、本机却没有五笔码表的人（{@link KeyboardScheme#wubiTableWanted}）：弹窗说明为什么要下载、要多大，用户同意后下载 wubi 资源包（{@link ResourcePackService#requestOnBehalf}），没联网时提示一句，进度和结果在通知里。
     *
     * <p>full 和拼音版的瘦包不带五笔码表。升级时 Bootstrap 把旧安装包留下的码表收编为资源包；收编不了的五笔用户（安装包换了词库锁、旧码表对不上，以后每次词库锁改了 `msime-wubi.db` 都会这样，已装的 wubi 资源包也随之过期），以及在首启引导里选了五笔的新用户，码表到来之前键盘不列出五笔，host-api 把五笔回退到别的方案，所以在设置应用回到前台时替他们问一次。词库自动更新已经下载过同一份码表时，共享层从那里复制，不联网。只在准备完成（{@link FirstRunPreparation.State#READY}）之后判断：准备途中收编正把码表从资源目录挪进资源包，这时两处都看不到它。收编遇到暂时性失败、码表正在 pending 目录里等下次启动时也不问（{@link Bootstrap#adoptionPending}）。首启引导和开屏还在前面时不弹，等回到这个页面再判断；升级后的临时说明（{@link InstallNotice}）显示着时也不弹，等它关掉再判断。
     */
    private void requestWubiTableIfWanted() {
        if (FirstRunPreparation.state() != FirstRunPreparation.State.READY || WUBI_TABLE_REQUESTED.get()
                || themeWorker.isShutdown()) return;
        themeWorker.execute(() -> {
            AppEdition edition = AppEdition.current();
            if (!edition.offers("wubi")) return;
            JSONObject snapshot = HostStore.prepared(this) ? HostStore.loadPreferences(this) : null;
            JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
            if (preferences == null) return;
            String scheme = InputViewValuePolicy.textOr(preferences, "scheme", "");
            String lastChinese = InputViewValuePolicy.textOr(preferences, "last_chinese_scheme", "");
            java.util.List<String> enabledIds = SchemePreferences.enabledIds(preferences);
            // 先按偏好排除不用五笔的人，免得每次回到前台都经 JNI 列一遍资源包。
            if (!KeyboardScheme.wubiTableWanted(scheme, lastChinese, enabledIds, java.util.Set.of(), edition)) return;
            if (Bootstrap.adoptionPending(getFilesDir(), ResourcePacks.WUBI)) return;
            ResourcePacks.Pack pack = ResourcePacks.pack(getFilesDir(), ResourcePacks.WUBI);
            if (pack == null) return;
            java.util.Set<String> packs = KeyboardScheme.availablePacks(
                id -> ResourcePacks.WUBI.equals(id) && pack.installed(),
                HostOptionsPolicy.readOption(getFilesDir(), "resources"));
            if (!KeyboardScheme.wubiTableWanted(scheme, lastChinese, enabledIds, packs, edition)) return;
            String reason = "outdated".equals(pack.state())
                ? "词库更新了五笔码表，本机那份旧的已经不能用，在下载新的之前键盘里没有五笔。"
                : "你在用五笔，但这个版本的安装包不带五笔码表，下载之前键盘里没有五笔。";
            runOnUiThread(() -> {
                if (isFinishing() || isDestroyed() || !getLifecycle().getCurrentState().isAtLeast(
                        androidx.lifecycle.Lifecycle.State.RESUMED)) return;
                if (ViewPolicy.isVisible(findViewById(R.id.home_intro)) || !OnboardingActivity.seen(this)
                        || installNoticeShowing) return;
                ResourcePackService.Status status = ResourcePackService.status(ResourcePacks.WUBI);
                if (status != null && status.running()) return;
                if (!WUBI_TABLE_REQUESTED.compareAndSet(false, true)) return;
                ResourcePackService.requestOnBehalf(this, ResourcePacks.WUBI, pack.size(), reason);
            });
        });
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
        if (ViewPolicy.isVisible(intro)) return;
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
        int tab = page != null ? SettingsNavigator.tab(page, request.args) : request.tab;
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
     * 开场：本季底色上光晕浮起，水杉标志（不垫圆盘，标志本身就是外框）随后旋入，折线自己写一遍，名字与拉丁名升上来，底部进度条 2.4 秒走满，停满 2.8 秒或被轻点后让开。
     *
     * <p>Drawn by the app rather than by the platform's splash screen. Timing follows the design's keyframes: msCircIn for the container the mark sits in, which no longer paints a disc (0.65 s), msLogoIn for the mark (0.6 s from 0.25 s, turning in from -30°), the stroke drawn by the animated vector (the design's msRipple ring is not drawn: nothing goes around the mark), the name and the Latin line fading up at 0.9 and 1.1 s, and msBreath on the halo from 1.4 s.
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
        pop(glow, 800, 0, MotionCurves.EASE);
        View disc = findViewById(R.id.home_intro_disc);
        pop(disc, 650, 0, MotionCurves.POP);
        ImageView mark = findViewById(R.id.home_intro_mark);
        mark.animate().cancel();
        mark.setAlpha(0f);
        mark.setScaleX(0.6f);
        mark.setScaleY(0.6f);
        mark.setRotation(-30f);
        mark.animate().alpha(1f).scaleX(1f).scaleY(1f).rotation(0f)
            .setDuration(600).setStartDelay(250).setInterpolator(MotionCurves.POP).start();
        if (mark.getDrawable() instanceof Animatable animatable) {
            animatable.stop();
            animatable.start();
        }
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
        breath.setInterpolator(MotionCurves.EASE);
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
        if (!ViewPolicy.isVisible(intro) || !intro.isClickable()) return;
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
        view.setTranslationY(DimensionPolicy.pixels(this, 10));
        view.animate().alpha(1f).translationY(0f)
            .setDuration(500).setStartDelay(delay).setInterpolator(MotionCurves.EASE).start();
    }

    @Override protected void onDestroy() {
        FirstRunPreparation.stopObserving(dictionaryPreparation);
        stopBreath();
        themeWorker.shutdownNow();
        super.onDestroy();
    }

    @Override protected void onSaveInstanceState(@NonNull Bundle state) {
        super.onSaveInstanceState(state);
        state.putInt(STATE_TAB, selected);
        state.putBoolean(STATE_ONBOARDING_AFTER_INTRO, onboardingAfterIntro);
    }

    /** 切到底部的某个 tab，别的 tab 里的行可以这样跳过去。 */
    public void openTab(int itemId) {
        if (tabs != null) tabs.setSelectedItemId(itemId);
    }

    /** 当前 tab 的下标，取值是 `HostDeepLink.TAB_*`。 */
    int selectedTabIndex() {
        int index = tabIndex(selected);
        return index == HostDeepLink.NO_TAB ? HostDeepLink.TAB_SETTINGS : index;
    }

    /** 底部导航项对应的 `HostDeepLink.TAB_*` 下标；不是现在的底部导航项时为 `HostDeepLink.NO_TAB`。 */
    private static int tabIndex(int itemId) {
        for (int i = 0; i < TAB_IDS.length; i++) {
            if (TAB_IDS[i] == itemId) return i;
        }
        return HostDeepLink.NO_TAB;
    }

    /** 按下标切 tab（`HostDeepLink.TAB_*`）；切换时清空回退栈。 */
    void selectTabIndex(int index) {
        if (!HostDeepLink.isTab(index)) return;
        openTab(TAB_IDS[index]);
    }

    /**
     * 底部导航的选中项变了：换 tab 之前先把详情页全部弹出。
     *
     * <p>这次弹栈不播回退动画。回退栈记录带着详情页的淡出和下面 tab 根页的淡入，照常播的话，根页会在接下来的切 tab 里一边淡入一边等着被藏起来，和目标 tab 的淡入叠成重影（#10092）。所以只让目标 tab 淡入；系统返回键、再点当前 tab 回根页时的回退动画不变。
     */
    private void selectTab(int itemId) {
        if (itemId != selected) {
            // popBackStackImmediate 是同步的，标记只覆盖这一次弹栈，下面 show() 的切 tab 淡入照常。
            clearingForTabSwitch = true;
            clearStack();
            clearingForTabSwitch = false;
        }
        show(itemId);
    }

    /** 是否正处在换 tab 前清栈的那一次同步弹栈里。 */
    boolean clearingForTabSwitch() {
        return clearingForTabSwitch;
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

    /** AI 对话压在栈顶时收起底部导航：它是二级页，返回走页面顶部；导航留着的话，键盘弹起时会夹在输入栏和键盘之间。弹出对话页后导航回来。 */
    private void updateTabsVisibility() {
        if (tabs == null) return;
        ViewPolicy.setVisible(tabs, WindowInsetsPolicy.shouldShowHomeTabs(imeVisible,
            SettingsNavigator.visibleIn(getSupportFragmentManager()) instanceof ChatFragment));
    }

    /**
     * 打开 AI 对话二级页并弹出键盘，供设置首页的「试用键盘」使用。
     */
    void openChat() {
        openChatSecondary(true);
    }

    void openChatSecondary(boolean focus) {
        SettingsNavigator.open(this, PageId.CHAT, null);
        if (focus && SettingsNavigator.visibleIn(getSupportFragmentManager()) instanceof ChatFragment chat) {
            chat.focusInput();
        }
    }

    /** 打开某一类社区作品的完整列表：皮肤和词库切到社区 tab，其余压在当前页面上面（见 {@link CommunityFragment#browse}）。 */
    public void openCommunity(CommunityRequest.Kind kind) {
        CommunityFragment.browse(this, kind);
    }

    /** 切到社区 tab 的根页并选中这一类作品的分类；只由 {@link SettingsNavigator} 调用。先选分类再切 tab：切过去时直接是要的那一类，第一次打开时也不会先把另一类的列表载入一遍。 */
    void openCommunityTab(CommunityRequest.Kind kind) {
        if (getSupportFragmentManager().findFragmentByTag(tag(R.id.tab_community)) instanceof CommunityTabPage page) {
            page.select(kind);
        } else {
            communityKind = kind;
        }
        selectTabIndex(HostDeepLink.TAB_COMMUNITY);
        clearStack();
    }

    /** 详情页创建时给导航一个统一的即时反馈；内容读盘仍由各页面自己管理。 */
    void setDetailLoading(boolean loading) {
        View overlay = findViewById(R.id.home_detail_loading);
        if (overlay == null) return;
        if (hideDetailLoading != null) {
            overlay.removeCallbacks(hideDetailLoading);
            hideDetailLoading = null;
        }
        if (loading) {
            overlay.setAlpha(0f);
            ViewPolicy.show(overlay);
            overlay.animate().alpha(1f).setStartDelay(120).setDuration(160).start();
        } else {
            Runnable hide = () -> {
                ViewPolicy.hide(overlay);
                overlay.setAlpha(1f);
                hideDetailLoading = null;
            };
            hideDetailLoading = hide;
            overlay.animate().cancel();
            // 统一给每个页面留出一小段可感知的反馈，避免快页面因为创建太快而像闪屏。
            overlay.postDelayed(() -> {
                if (hideDetailLoading != hide) return;
                overlay.animate().alpha(0f).setDuration(120).withEndAction(hide).start();
            }, 220);
        }
    }

    private void show(int itemId) {
        boolean switching = selected != itemId;
        selected = itemId;
        updateBack();
        FragmentManager manager = getSupportFragmentManager();
        Fragment target = manager.findFragmentByTag(tag(itemId));
        if (switching || target == null || rebuild == itemId) setDetailLoading(true);
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
        // Existing kept tabs do not emit a new view-created callback, so finish their global
        // transition loading here as well. New pages also use the lifecycle callback above; the
        // delayed hide keeps the same visible animation for both paths.
        if (target != null && rebuild != itemId) setDetailLoading(false);
    }

    private Fragment create(int itemId) {
        if (itemId == R.id.tab_community) {
            CommunityTabPage page = new CommunityTabPage();
            if (communityKind != null) page.select(communityKind);
            communityKind = null;
            return DetailPage.asTabRoot(page);
        }
        if (itemId == R.id.tab_statistics) return new StatisticsFragment();
        if (itemId == R.id.tab_account) return new AccountFragment();
        return new KeyboardFragment();
    }

    private static String tag(int itemId) { return "home-tab-" + itemId; }
}
