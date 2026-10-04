package app.msime.android;

import java.util.ArrayDeque;
import java.util.List;
import java.util.Queue;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

public final class CandidateTranslationStoreSmoke {
    public static void main(String[] args) throws Exception {
        FakeScheduler scheduler = new FakeScheduler();
        CountDownLatch started = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        ExecutorService worker = Executors.newSingleThreadExecutor();
        AtomicInteger arrivals = new AtomicInteger();
        CandidateTranslationStore.Service service = (texts, target) -> {
            started.countDown();
            release.await(2, TimeUnit.SECONDS);
            return List.of("stale translation");
        };
        CandidateTranslationStore store = new CandidateTranslationStore(
            service, worker, scheduler, generation -> arrivals.incrementAndGet());
        try {
            store.refresh(List.of("你好"), List.of("en"), 4);
            scheduler.runDelayed();
            check(started.await(2, TimeUnit.SECONDS), "translation request started");
            store.clear();
            release.countDown();
            worker.shutdown();
            check(worker.awaitTermination(2, TimeUnit.SECONDS), "translation worker stopped");
            scheduler.runPosted();
            check(store.gloss("你好", "en") == null, "stale translation was discarded");
            check(arrivals.get() == 0, "stale translation did not notify the host");
        } finally {
            release.countDown();
            worker.shutdownNow();
        }

        FakeScheduler retryScheduler = new FakeScheduler();
        ExecutorService retryWorker = Executors.newSingleThreadExecutor();
        AtomicInteger retryCalls = new AtomicInteger();
        CountDownLatch firstRetryCall = new CountDownLatch(1);
        CountDownLatch releaseRetryCall = new CountDownLatch(1);
        CountDownLatch secondRetryCall = new CountDownLatch(1);
        CandidateTranslationStore retryStore = new CandidateTranslationStore(
            (texts, target) -> {
                if (retryCalls.incrementAndGet() == 1) {
                    firstRetryCall.countDown();
                    releaseRetryCall.await(2, TimeUnit.SECONDS);
                } else {
                    secondRetryCall.countDown();
                }
                return List.of("retry translation");
            }, retryWorker, retryScheduler, generation -> { });
        try {
            retryStore.refresh(List.of("你好"), List.of("en"), 4);
            retryScheduler.runDelayed();
            check(firstRetryCall.await(2, TimeUnit.SECONDS), "first retry request started");
            retryStore.refresh(List.of("你好"), List.of("en"), 4);
            retryScheduler.runDelayed();
            releaseRetryCall.countDown();
            check(secondRetryCall.await(2, TimeUnit.SECONDS),
                "cancelled request can retry with the same signature");
        } finally {
            releaseRetryCall.countDown();
            retryWorker.shutdownNow();
        }

        FakeScheduler normalizationScheduler = new FakeScheduler();
        ExecutorService normalizationWorker = Executors.newSingleThreadExecutor();
        AtomicInteger normalizationArrivals = new AtomicInteger();
        CandidateTranslationStore normalizedStore = new CandidateTranslationStore(
            (texts, target) -> List.of(" \u00a0hello world\n\t"), normalizationWorker,
            normalizationScheduler, generation -> normalizationArrivals.incrementAndGet());
        try {
            normalizedStore.refresh(List.of("你好"), List.of("en"), 9);
            normalizationScheduler.runDelayed();
            normalizationWorker.shutdown();
            check(normalizationWorker.awaitTermination(2, TimeUnit.SECONDS),
                "normalization worker stopped");
            normalizationScheduler.runPosted();
            check("hello world".equals(normalizedStore.gloss("你好", "en")),
                "translation whitespace was normalized");
            check(normalizationArrivals.get() == 1, "normalized translation notified the host");
        } finally {
            normalizationWorker.shutdownNow();
        }

        FakeScheduler collisionScheduler = new FakeScheduler();
        ExecutorService collisionWorker = Executors.newSingleThreadExecutor();
        AtomicInteger collisionCalls = new AtomicInteger();
        CountDownLatch firstCollisionCall = new CountDownLatch(1);
        CountDownLatch secondCollisionCall = new CountDownLatch(1);
        CandidateTranslationStore collisionStore = new CandidateTranslationStore(
            (texts, target) -> {
                if (collisionCalls.incrementAndGet() == 1) firstCollisionCall.countDown();
                else secondCollisionCall.countDown();
                return texts.stream().map(text -> text + " translation").toList();
            }, collisionWorker, collisionScheduler, generation -> { });
        try {
            collisionStore.refresh(List.of("甲|乙"), List.of("en"), 6);
            collisionScheduler.runDelayed();
            check(firstCollisionCall.await(2, TimeUnit.SECONDS), "first collision request started");
            collisionScheduler.runPosted();

            collisionStore.refresh(List.of("甲", "乙"), List.of("en"), 6);
            collisionScheduler.runDelayed();
            check(secondCollisionCall.await(2, TimeUnit.SECONDS), "second collision request started");
            collisionScheduler.runPosted();
            check(collisionCalls.get() == 2, "word separators do not collide");
        } finally {
            collisionWorker.shutdownNow();
        }

        CountDownLatch firstCapacityResponse = new CountDownLatch(1);
        FakeScheduler capacityScheduler = new FakeScheduler() {
            @Override public void post(Runnable action) {
                super.post(action);
                firstCapacityResponse.countDown();
            }
        };
        ExecutorService capacityWorker = Executors.newSingleThreadExecutor();
        AtomicInteger capacityCalls = new AtomicInteger();
        CountDownLatch firstCapacityCall = new CountDownLatch(1);
        CountDownLatch secondCapacityCall = new CountDownLatch(1);
        List<String> capacityWords = new java.util.ArrayList<>();
        for (int index = 0; index < 257; index++) capacityWords.add("合成词" + index);
        CandidateTranslationStore capacityStore = new CandidateTranslationStore(
            (texts, target) -> {
                if (capacityCalls.incrementAndGet() == 1) firstCapacityCall.countDown();
                else secondCapacityCall.countDown();
                return texts.stream().map(text -> text + " translation").toList();
            }, capacityWorker, capacityScheduler, generation -> { });
        try {
            capacityStore.refresh(capacityWords, List.of("en"), 7);
            capacityScheduler.runDelayed();
            check(firstCapacityCall.await(2, TimeUnit.SECONDS), "capacity request started");
            check(firstCapacityResponse.await(2, TimeUnit.SECONDS), "capacity response posted");
            capacityScheduler.runPosted();
            capacityStore.refresh(List.of(capacityWords.get(0)), List.of("en"), 8);
            capacityScheduler.runDelayed();
            check(secondCapacityCall.await(2, TimeUnit.SECONDS),
                "evicted translation can be requested again");
        } finally {
            capacityWorker.shutdownNow();
        }
        check(capacityCalls.get() == 2, "translation cache evicts old entries");
        System.out.println("Android candidate translation store: stale request fencing passed");
    }

    private static class FakeScheduler implements CandidateTranslationStore.Scheduler {
        private final Queue<Runnable> delayed = new ArrayDeque<>();
        private final Queue<Runnable> posted = new ArrayDeque<>();

        @Override public void post(Runnable action) { posted.add(action); }
        @Override public void postDelayed(Runnable action, long delayMillis) { delayed.add(action); }
        @Override public void removeCallbacks(Runnable action) { delayed.remove(action); }

        void runDelayed() {
            Runnable action = delayed.poll();
            if (action == null) throw new AssertionError("missing delayed request");
            action.run();
        }

        void runPosted() {
            Runnable action;
            while ((action = posted.poll()) != null) action.run();
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
