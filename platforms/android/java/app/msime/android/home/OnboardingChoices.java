package app.msime.android.home;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.Looper;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.FirstRunPreparation;
import app.msime.android.KeyboardScheme;
import app.msime.android.SchemePreferences;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 引导页上选好、还没写进共享偏好的方案和「显示译文」。
 *
 * <p>首次安装时偏好存储的目录要等词库准备完成才定下来（{@link HostStore#directory} 读的是准备写下的运行配置），引导页的方案一步恰好落在准备的那几十秒里。这里把用户的选择先记在本应用自己的 SharedPreferences 里（与「引导已看过」同一个文件），界面立即按它显示；偏好可读后由 {@link #sync} 写进共享偏好，写成功才清掉。选择不跟页面走：用户离开引导、页面重建、进程在准备途中被杀，选择都还在，{@link #watch} 在本进程的词库准备完成时再写一次，下次打开应用时 {@link HomeActivity} 也会重新挂上它。
 *
 * <p>所有写入都在同一条工作线程上排队，两次点击、页面和进程级的触发不会并发地读改写同一份偏好。
 */
final class OnboardingChoices {
    private static final String PENDING_SCHEME = "pending_scheme";
    private static final String PENDING_GLOSS = "pending_gloss";
    /** 键盘读这个键决定是否在候选下方加一行英文释义，共享核心的默认值是关。 */
    static final String GLOSS = "candidate_english_gloss";
    /** 保存被拒（多半是键盘恰好也写了偏好、修订号对不上）时重新读取再试的次数。 */
    private static final int ATTEMPTS = 3;
    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(runnable -> {
        Thread thread = new Thread(runnable, "msime-onboarding-choices");
        thread.setDaemon(true);
        return thread;
    });
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final AtomicBoolean WATCHING = new AtomicBoolean();
    /** 记下选择与写成功后清掉选择互斥：清之前要确认它还是刚写进去的那个，用户在写入途中又改了就留着下一次写。 */
    private static final Object LOCK = new Object();

    private OnboardingChoices() {}

    /** 一次 {@link #sync} 的结果。 */
    enum Outcome {
        /** 没有待保存的选择。 */
        NOTHING,
        /** 待保存的选择已经写进共享偏好（或与已存的相同，不必写）。 */
        SAVED,
        /** 偏好还读不到，选择留着，之后再写。 */
        WAITING,
        /** 偏好读得到但写不进去，选择已放弃，键盘保留原来的设置。 */
        FAILED,
        /** 读写时出了意外的异常：选择留着，调用方可以稍后再试，也会在下次准备完成或下次启动时再写。 */
        ERROR
    }

    /**
     * @param snapshot 这次读到的最新快照（`revision` 和 `preferences`）；读不到时为 null
     */
    record Result(Outcome outcome, @Nullable JSONObject snapshot) {}

    @Nullable static KeyboardScheme pendingScheme(Context context) {
        return OnboardingChoicePolicy.pendingScheme(
            store(context).getString(PENDING_SCHEME, null), AppEdition.current());
    }

    @Nullable static Boolean pendingGloss(Context context) {
        SharedPreferences store = store(context);
        return store.contains(PENDING_GLOSS) ? store.getBoolean(PENDING_GLOSS, false) : null;
    }

    static void rememberScheme(Context context, KeyboardScheme scheme) {
        synchronized (LOCK) {
            store(context).edit().putString(PENDING_SCHEME, scheme.preferenceId()).apply();
        }
    }

    static void rememberGloss(Context context, boolean on) {
        synchronized (LOCK) {
            store(context).edit().putBoolean(PENDING_GLOSS, on).apply();
        }
    }

    /** 偏好里存的方案入口，与设置页的方案选择读法相同；本版本没有的方案按 {@link KeyboardScheme#fallback}。 */
    static KeyboardScheme storedScheme(JSONObject preferences, AppEdition edition) {
        return SchemePreferences.storedScheme(preferences, edition);
    }

    /**
     * 在本进程余下的时间里，每当词库准备完成就把待保存的选择写进去。重复调用只挂一次；监听器挂在进程上而不是页面上，用户在准备完成前就离开引导，选择照样会写进去。
     */
    static void watch(Context context) {
        if (!WATCHING.compareAndSet(false, true)) return;
        Context application = context.getApplicationContext();
        FirstRunPreparation.observe(state -> {
            if (state == FirstRunPreparation.State.READY) sync(application, null);
        });
    }

    /**
     * 在工作线程上读一次偏好，有待保存的选择就写进去，再把结果交给主线程上的 `done`（可以为 null）。没有选择时也读，页面靠它拿到最新快照。
     */
    static void sync(Context context, @Nullable Consumer<Result> done) {
        Context application = context.getApplicationContext();
        WORKER.execute(() -> {
            Result result;
            try {
                result = syncNow(application, done != null);
            } catch (RuntimeException | LinkageError error) {
                // 共享宿主的调用在 HostStore 里已经兜住；这里是线程边界，未预料的异常不能让应用崩掉，选择留着下次再写。
                result = new Result(Outcome.ERROR, null);
            }
            if (done == null) return;
            Result answer = result;
            MAIN.post(() -> done.accept(answer));
        });
    }

    /** @param wantSnapshot 调用方要最新快照；不要时（进程级的 {@link #watch}）没有待保存的选择就不读偏好 */
    private static Result syncNow(Context context, boolean wantSnapshot) {
        AppEdition edition = AppEdition.current();
        SharedPreferences store = store(context);
        if (!wantSnapshot && !store.contains(PENDING_SCHEME) && !store.contains(PENDING_GLOSS))
            return new Result(Outcome.NOTHING, null);
        JSONObject snapshot = HostStore.loadPreferences(context);
        String scheme = null;
        Boolean gloss = null;
        for (int attempt = 0; attempt < ATTEMPTS; attempt++) {
            synchronized (LOCK) {
                scheme = store.getString(PENDING_SCHEME, null);
                gloss = store.contains(PENDING_GLOSS) ? store.getBoolean(PENDING_GLOSS, false) : null;
            }
            if (scheme == null && gloss == null) return new Result(Outcome.NOTHING, snapshot);
            JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
            if (preferences == null) return new Result(Outcome.WAITING, snapshot);
            KeyboardScheme schemeChange = OnboardingChoicePolicy.schemeToWrite(
                scheme, storedScheme(preferences, edition), edition);
            Boolean glossChange = OnboardingChoicePolicy.glossToWrite(gloss, preferences.optBoolean(GLOSS, false));
            if (schemeChange == null && glossChange == null) {
                clear(store, scheme, gloss);
                return new Result(Outcome.SAVED, snapshot);
            }
            JSONObject pending = edited(snapshot, schemeChange, glossChange);
            if (pending == null) {
                clear(store, scheme, gloss);
                return new Result(Outcome.FAILED, snapshot);
            }
            JSONObject saved = HostStore.savePreferences(context, pending);
            JSONObject fresh = HostStore.loadPreferences(context);
            if (saved != null) {
                clear(store, scheme, gloss);
                return new Result(Outcome.SAVED, fresh != null ? fresh : snapshot);
            }
            // 被拒多半是键盘在这期间也写了偏好：按刚读到的新修订号再算一遍。
            if (fresh != null) snapshot = fresh;
        }
        // 几次都写不进去：放弃这次的选择，页面按偏好里实际存的显示，不让卡片一直停在没生效的选择上。
        clear(store, scheme, gloss);
        return new Result(Outcome.FAILED, snapshot);
    }

    /** 快照改好要写的那几个键后的副本；方案走 {@link SchemePreferences#withScheme}，与设置页写出的完全相同，五笔版本（`wubi_profile`）保持原样。 */
    @Nullable private static JSONObject edited(JSONObject snapshot, @Nullable KeyboardScheme scheme,
            @Nullable Boolean gloss) {
        try {
            JSONObject pending = scheme == null ? new JSONObject(snapshot.toString())
                : SchemePreferences.withScheme(snapshot, scheme, null);
            if (pending == null) return null;
            if (gloss != null) pending.getJSONObject("preferences").put(GLOSS, gloss.booleanValue());
            return pending;
        } catch (JSONException error) {
            return null;
        }
    }

    /** 清掉刚处理过的选择；用户在这期间又改了的那一项留着。 */
    private static void clear(SharedPreferences store, @Nullable String scheme, @Nullable Boolean gloss) {
        synchronized (LOCK) {
            SharedPreferences.Editor editor = store.edit();
            if (scheme != null && scheme.equals(store.getString(PENDING_SCHEME, null))) editor.remove(PENDING_SCHEME);
            if (gloss != null && store.contains(PENDING_GLOSS)
                    && gloss == store.getBoolean(PENDING_GLOSS, false)) editor.remove(PENDING_GLOSS);
            editor.apply();
        }
    }

    private static SharedPreferences store(Context context) {
        return context.getSharedPreferences(OnboardingActivity.STORE, Context.MODE_PRIVATE);
    }
}
