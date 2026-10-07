import app.msime.android.AiPolishClient;
import app.msime.android.AiPolishConfiguration;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

public final class AiPolishClientSmoke {
    static void check(boolean value) { if (!value) throw new AssertionError(); }

    public static void main(String[] arguments) throws Exception {
        AiPolishConfiguration config = new AiPolishConfiguration(
            "https://Fixture.Invalid/v1/chat/completions", "fixture-model", "保留原意", "fixture-key");
        check(config.credentialOrigin().equals("https://fixture.invalid:443"));
        check(config.destination().equals("https://fixture.invalid"));
        check(AiPolishConfiguration.credentialOrigin(
            "https://fixture.invalid/another/path").equals(config.credentialOrigin()));
        check(!AiPolishConfiguration.credentialOrigin(
            "https://fixture.invalid:444/v1/chat/completions").equals(config.credentialOrigin()));
        check(config.toString().contains("fixture-model") && !config.toString().contains("fixture-key"));
        AiPolishConfiguration overridden = config.withPrompt("reply prompt");
        check(overridden.prompt().equals("reply prompt") && overridden.credentialOrigin().equals(config.credentialOrigin()));
        check(overridden.toString().contains("fixture-model") && !overridden.toString().contains("fixture-key"));
        check(AiPolishConfiguration.promptSlotKey("custom_2").equals("prompt_custom_2"));
        check(AiPolishConfiguration.promptSlotKey("custom_3").equals("prompt_custom_3"));
        check(AiPolishConfiguration.promptSlotKey("").equals("prompt_custom_1"));
        check(AiPolishConfiguration.promptSlotKey("custom").equals("prompt_custom_1"));
        check(AiPolishConfiguration.acceptableText("𠮷".repeat(10_000)));
        check(!AiPolishConfiguration.acceptableText("a".repeat(10_001)));
        check(!AiPolishConfiguration.acceptableText("\ud800"));
        expectInvalid("https://fixture.invalid/" + "界".repeat(700), "fixture-model",
            "保留原意", "fixture-key", "endpoint uses a UTF-8 byte bound");
        expectInvalid("https://fixture.invalid/v1/chat/completions", "😀".repeat(200),
            "保留原意", "fixture-key", "model uses a UTF-8 byte bound");
        expectInvalid("https://fixture.invalid/v1/chat/completions", "fixture-model",
            "😀".repeat(5_000), "fixture-key", "prompt uses a UTF-8 byte bound");
        expectInvalid("https://fixture.invalid/v1/chat/completions", "fixture-model",
            "保留原意", "😀".repeat(5_000), "token uses a UTF-8 byte bound");

        CountDownLatch firstStarted = new CountDownLatch(1);
        CountDownLatch releaseFirst = new CountDownLatch(1);
        CountDownLatch cancelStarted = new CountDownLatch(1);
        AtomicInteger callbacks = new AtomicInteger();
        AtomicReference<String> result = new AtomicReference<>();
        try (AiPolishClient client = new AiPolishClient((configuration, text, cancellation) -> {
            if (text.equals("first")) {
                firstStarted.countDown();
                try { releaseFirst.await(2, TimeUnit.SECONDS); }
                catch (InterruptedException error) { Thread.currentThread().interrupt(); }
                return "late";
            }
            if (text.equals("cancel")) {
                cancelStarted.countDown();
                while (!cancellation.cancelled()) Thread.onSpinWait();
                return "cancelled-result";
            }
            return "second-result";
        })) {
            client.request(config, "first", (generation, value, error) -> callbacks.incrementAndGet());
            check(firstStarted.await(2, TimeUnit.SECONDS));
            CountDownLatch completed = new CountDownLatch(1);
            client.request(config, "second", (generation, value, error) -> {
                callbacks.incrementAndGet();
                result.set(value);
                check(error == null);
                completed.countDown();
            });
            releaseFirst.countDown();
            check(completed.await(2, TimeUnit.SECONDS));
            check(callbacks.get() == 1);
            check("second-result".equals(result.get()));

            CountDownLatch cancelledCallback = new CountDownLatch(1);
            AiPolishClient.Operation cancelled = client.request(config, "cancel",
                (generation, value, error) -> cancelledCallback.countDown());
            check(cancelStarted.await(2, TimeUnit.SECONDS));
            cancelled.cancel();
            check(!cancelledCallback.await(100, TimeUnit.MILLISECONDS));
        }
    }

    private static void expectInvalid(String endpoint, String model, String prompt, String token,
            String message) {
        try {
            new AiPolishConfiguration(endpoint, model, prompt, token);
            throw new AssertionError(message);
        } catch (IllegalArgumentException expected) {
            // expected
        }
    }
}
