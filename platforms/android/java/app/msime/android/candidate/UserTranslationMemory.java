package app.msime.android;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.LongSupplier;

/**
 * 用户自己的翻译服务的负缓存和退避，一个 {@link UserTranslationClient}（也就是一种服务加一组凭据）一份，换服务或凭据时随客户端一起丢掉。
 *
 * <p>为什么要有它：键盘每次重绘都会让 store 重新排一次请求，而 store 只缓存非空译文。没有这一层，服务回答「没有译文」的词、或者服务暂时失败的词，会在同一页的每次重绘里被重新发一遍，持续消耗用户自己的额度。
 *
 * <ul>
 * <li>服务回答了但没有译文（共享层判定不是失败）：记下这个词，同一服务下不再问。</li>
 * <li>单个词的回复是失败（限流、报错正文）：这个词 {@link #BACKOFF_MILLIS} 内不再问，之后再试。</li>
 * <li>整个请求没有回复（连不上、超时、非 2xx）：整个服务 {@link #BACKOFF_MILLIS} 内不再发请求。这类失败几乎总是服务整体的问题（地址错、断网、密钥失效、额度用完），逐词再试只会把每个词都超时一遍。</li>
 * </ul>
 *
 * <p>只在 store 的单个 worker 线程上用，方法仍加了锁，免得以后换线程模型时悄悄出错。不依赖 Android 和 org.json，JVM 冒烟直接测它。
 */
public final class UserTranslationMemory {
    /** 失败后多久再试。 */
    public static final long BACKOFF_MILLIS = 30_000;
    /** 负缓存和逐词退避各自最多记多少个词，满了丢最早的。 */
    static final int MAX_ENTRIES = 512;

    private final LongSupplier clock;
    private final Map<String, Boolean> answeredEmpty = new LinkedHashMap<>(64, 0.75f, true);
    private final Map<String, Long> failedUntil = new LinkedHashMap<>(16, 0.75f, true);
    private long serviceUntil = Long.MIN_VALUE;

    /** @param clock 单调时钟，毫秒；键盘用 {@code SystemClock.elapsedRealtime}，冒烟用假时钟。 */
    public UserTranslationMemory(LongSupplier clock) {
        this.clock = clock;
    }

    /** 这个词现在能不能问：没有因为「已回答为空」被记下，服务和这个词都不在退避期。 */
    public synchronized boolean shouldAsk(String target, String word) {
        long now = clock.getAsLong();
        if (now < serviceUntil) return false;
        String key = key(target, word);
        if (answeredEmpty.containsKey(key)) return false;
        Long until = failedUntil.get(key);
        if (until == null) return true;
        if (now < until) return false;
        failedUntil.remove(key);
        return true;
    }

    /** 整个服务现在是否在退避期，在退避期就一个请求都不发。 */
    public synchronized boolean serviceBackingOff() {
        return clock.getAsLong() < serviceUntil;
    }

    /** 服务对这个词给出了回答；{@code empty} 为 true 表示回答里没有译文，以后不再问。 */
    public synchronized void answered(String target, String word, boolean empty) {
        String key = key(target, word);
        failedUntil.remove(key);
        if (!empty) return;
        if (!answeredEmpty.containsKey(key) && answeredEmpty.size() >= MAX_ENTRIES)
            answeredEmpty.remove(answeredEmpty.keySet().iterator().next());
        answeredEmpty.put(key, Boolean.TRUE);
    }

    /** 这个词的回复是失败：{@link #BACKOFF_MILLIS} 内不再问它。 */
    public synchronized void failed(String target, String word) {
        String key = key(target, word);
        if (!failedUntil.containsKey(key) && failedUntil.size() >= MAX_ENTRIES)
            failedUntil.remove(failedUntil.keySet().iterator().next());
        failedUntil.put(key, clock.getAsLong() + BACKOFF_MILLIS);
    }

    /** 请求整个没有得到回复：{@link #BACKOFF_MILLIS} 内这个服务一个请求都不发。 */
    public synchronized void unreachable() {
        serviceUntil = clock.getAsLong() + BACKOFF_MILLIS;
    }

    private static String key(String target, String word) {
        return target.length() + ":" + target + "|" + word;
    }
}
