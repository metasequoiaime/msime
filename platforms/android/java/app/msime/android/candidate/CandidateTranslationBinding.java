package app.msime.android;

/**
 * 账号译文的缓存属于哪个账号。缓存里的译文在同一帧里和候选一起画出来，账号绑定在后台核对，不再每一代都等一次跨进程读取：
 *
 * <ul>
 * <li>键盘显示、换输入框、进程刚起来时还没核对过（{@link #reset}）：这时不画缓存里的账号译文，也不发请求，核对完成后再画；</li>
 * <li>核对过之后照常用缓存，超过 {@link #MAX_AGE_MILLIS} 没核对就在后台再核对一次（{@link #ensureFresh}），这期间仍用缓存；</li>
 * <li>核对发现账号变了：store 换绑定（同时清空），画面上的账号译文随之去掉；</li>
 * <li>整页请求在 worker 上发出前、回答到达后各读一次绑定（{@link #holds}），对不上就不发、不收，并让这里马上重新核对（{@link #invalidate}）。</li>
 * </ul>
 *
 * <p>读绑定是跨进程调用，只在 worker 上做；其余状态只在主线程上读写。
 */
public final class CandidateTranslationBinding {
    /** worker 上读当前账号绑定的键；provider 不可达时为 null。 */
    public interface Reader { String key(); }

    /** 在 worker 上跑一个任务；被拒时返回 false。worker 也可能悄悄丢掉排着的任务，所以核对超过 {@link #CHECK_TIMEOUT_MILLIS} 没回音就重发。 */
    public interface Worker { boolean execute(Runnable task); }

    public interface Main { void post(Runnable task); }

    public interface Clock { long now(); }

    /** 缓存本身（{@link CandidateTranslationStore}）。 */
    public interface Cache {
        /** 换绑定；绑定变了就清空缓存并返回 false。 */
        boolean bindTo(String key);
        boolean hasEntries();
    }

    /** 主线程上：核对完成。{@code cleared} 为真表示缓存因换账号被清空，画面上已有的账号译文要去掉。 */
    public interface Listener { void onChecked(boolean cleared); }

    public static final long MAX_AGE_MILLIS = 10_000;
    static final long CHECK_TIMEOUT_MILLIS = 1_000;

    private final Reader reader;
    private final Worker worker;
    private final Main main;
    private final Clock clock;
    private final Cache cache;
    private final Listener listener;
    private boolean usable;
    private long verifiedAt;
    private long epoch;
    private long checkStartedAt = Long.MIN_VALUE;

    public CandidateTranslationBinding(Reader reader, Worker worker, Main main, Clock clock,
                                       Cache cache, Listener listener) {
        this.reader = reader;
        this.worker = worker;
        this.main = main;
        this.clock = clock;
        this.cache = cache;
        this.listener = listener;
    }

    /** 缓存里的账号译文现在能不能画、能不能发请求：自上次 {@link #reset} 以来核对过、而且账号可以确认。 */
    public boolean usable() { return usable; }

    /** 键盘显示、换输入框、会话重建：在下一次核对完成之前不用缓存。正在路上的核对作废。 */
    public void reset() {
        usable = false;
        epoch++;
        checkStartedAt = Long.MIN_VALUE;
    }

    /** 每张新列表调用：没核对过就核对；核对过但超过 {@link #MAX_AGE_MILLIS} 就在后台再核对，期间照常用缓存。 */
    public void ensureFresh() {
        long now = clock.now();
        if (checking(now)) return;
        if (!usable || now - verifiedAt > MAX_AGE_MILLIS) check(now);
    }

    /** 请求前后发现绑定对不上：先停用缓存，马上重新核对。 */
    public void invalidate() {
        reset();
        check(clock.now());
    }

    /** worker 上：此刻的账号绑定是不是 {@code bound}。整页请求发出前和回答到达后各问一次。 */
    public boolean holds(String bound) {
        return bound != null && bound.equals(reader.key());
    }

    private boolean checking(long now) {
        return checkStartedAt != Long.MIN_VALUE && now - checkStartedAt < CHECK_TIMEOUT_MILLIS;
    }

    private void check(long now) {
        long requested = epoch;
        checkStartedAt = now;
        boolean queued = worker.execute(() -> {
            String key = reader.key();
            main.post(() -> finish(requested, key));
        });
        if (!queued) checkStartedAt = Long.MIN_VALUE;
    }

    private void finish(long requested, String key) {
        if (requested != epoch) return;
        checkStartedAt = Long.MIN_VALUE;
        // provider 不可达时证明不了缓存属于谁：不用缓存，也不发请求，下一张列表再核对。
        boolean accepted = CandidateTranslationPolicy.acceptsBinding(key, key);
        boolean hadEntries = cache.hasEntries();
        boolean same = cache.bindTo(accepted ? key : null);
        usable = accepted;
        verifiedAt = clock.now();
        listener.onChecked(!same && hadEntries);
    }
}
