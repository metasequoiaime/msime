package app.msime.android;

import android.content.Context;
import android.os.Handler;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutorService;

/** Debounced, display-only online candidate translation cache. All callbacks run on main. */
public final class CandidateTranslationStore {
    interface Scheduler {
        void post(Runnable action);
        void postDelayed(Runnable action, long delayMillis);
        void removeCallbacks(Runnable action);
    }

    public interface Service {
        List<String> translate(List<String> texts, String target) throws Exception;
    }
    public interface Listener {
        void onArrival(long generation);
    }
    public static final long QUIET_INTERVAL_MILLIS = 350;
    private final Service service;
    private final ExecutorService worker;
    private final Scheduler scheduler;
    private final Listener listener;
    private final Map<String, String> cache = new LinkedHashMap<>();
    private Runnable pending;
    private String signature;
    private long requestEpoch;

    public CandidateTranslationStore(Context context, ExecutorService worker, Handler main,
                                     Listener listener) {
        this(new BackendTranslationClient(context), worker, main, listener);
    }

    public CandidateTranslationStore(Service service, ExecutorService worker, Handler main,
                                     Listener listener) {
        this(service, worker, new HandlerScheduler(main), listener);
    }

    CandidateTranslationStore(Service service, ExecutorService worker, Scheduler scheduler,
                               Listener listener) {
        this.service = service;
        this.worker = worker;
        this.scheduler = scheduler;
        this.listener = listener;
    }

    public static boolean translatable(String value) {
        if (value == null || value.isEmpty()) return false;
        return value.codePoints().anyMatch(codePoint ->
            (codePoint >= 0x4E00 && codePoint <= 0x9FFF)
                || (codePoint >= 0x3400 && codePoint <= 0x4DBF)
                || (codePoint >= 0xF900 && codePoint <= 0xFAFF)
                || (codePoint >= 0x20000 && codePoint <= 0x3FFFF));
    }

    public String gloss(String word, String target) {
        return cache.get(key(target, word));
    }

    public void refresh(List<String> words, String target, long generation) {
        refresh(words, target == null ? List.of() : List.of(target), generation);
    }

    public void refresh(List<String> words, List<String> targets, long generation) {
        cancel();
        if (words == null || targets == null || targets.isEmpty()) return;
        ArrayList<String> requestedTargets = new ArrayList<>();
        for (String target : targets) {
            if (target != null && !target.isEmpty() && !requestedTargets.contains(target))
                requestedTargets.add(target);
        }
        if (requestedTargets.isEmpty()) return;
        ArrayList<String> wanted = new ArrayList<>();
        for (String word : words) {
            if (translatable(word) && !wanted.contains(word)) wanted.add(word);
        }
        if (wanted.isEmpty()) return;
        long epoch = requestEpoch;
        pending = () -> send(wanted, requestedTargets, generation, epoch);
        scheduler.postDelayed(pending, QUIET_INTERVAL_MILLIS);
    }

    public void cancel() {
        requestEpoch = requestEpoch == Long.MAX_VALUE ? 0 : requestEpoch + 1;
        if (pending != null) scheduler.removeCallbacks(pending);
        pending = null;
    }

    public void clear() {
        cancel();
        cache.clear();
        signature = null;
    }

    private void send(List<String> words, List<String> targets, long generation, long epoch) {
        pending = null;
        if (epoch != requestEpoch) return;
        String stamp = "targets=" + signature(targets) + "|generation=" + generation
            + "|words=" + signature(words);
        if (stamp.equals(signature)) return;
        Map<String, ArrayList<String>> requests = new LinkedHashMap<>();
        for (String target : targets) {
            ArrayList<String> missing = new ArrayList<>();
            for (String word : words) {
                if (!cache.containsKey(key(target, word))) missing.add(word);
            }
            if (!missing.isEmpty()) requests.put(target, missing);
        }
        if (requests.isEmpty()) return;
        signature = stamp;
        try {
            worker.execute(() -> {
                for (Map.Entry<String, ArrayList<String>> request : requests.entrySet()) {
                    String target = request.getKey();
                    ArrayList<String> missing = request.getValue();
                    try {
                        List<String> values = service.translate(missing, target);
                        scheduler.post(() -> absorb(epoch, target, generation, missing, values));
                    } catch (Exception ignored) {
                        // Optional display data must never disturb input or expose response text.
                    }
                }
            });
        } catch (RuntimeException ignored) {
            // A stopped worker is equivalent to an unavailable optional service.
        }
    }

    private static String signature(List<String> values) {
        StringBuilder result = new StringBuilder().append(values.size()).append(':');
        for (String value : values) {
            result.append(value.length()).append(':').append(value);
        }
        return result.toString();
    }

    private void absorb(long epoch, String target, long generation,
                        List<String> words, List<String> values) {
        if (epoch != requestEpoch) return;
        if (values == null || words.size() != values.size()) return;
        boolean arrived = false;
        for (int index = 0; index < words.size(); index++) {
            String value = values.get(index);
            value = trimWhitespace(value);
            if (value == null || value.isEmpty() || value.equals(words.get(index))
                    || TextPolicy.utf8Length(value) > 4096) continue;
            cache.put(key(target, words.get(index)), value);
            arrived = true;
        }
        if (arrived) listener.onArrival(generation);
    }

    /** Match Apple's whitespace/newline normalization before a gloss enters the cache. */
    private static String trimWhitespace(String value) {
        if (value == null || value.isEmpty()) return value;
        int start = 0;
        while (start < value.length()) {
            int codePoint = value.codePointAt(start);
            if (!Character.isWhitespace(codePoint) && !Character.isSpaceChar(codePoint)) break;
            start += Character.charCount(codePoint);
        }
        int end = value.length();
        while (end > start) {
            int codePoint = value.codePointBefore(end);
            if (!Character.isWhitespace(codePoint) && !Character.isSpaceChar(codePoint)) break;
            end -= Character.charCount(codePoint);
        }
        return value.substring(start, end);
    }

    private static final class HandlerScheduler implements Scheduler {
        private final Handler handler;

        HandlerScheduler(Handler handler) { this.handler = handler; }

        @Override public void post(Runnable action) { handler.post(action); }
        @Override public void postDelayed(Runnable action, long delayMillis) {
            handler.postDelayed(action, delayMillis);
        }
        @Override public void removeCallbacks(Runnable action) { handler.removeCallbacks(action); }
    }

    private static String key(String target, String word) { return target + "|" + word; }
}
