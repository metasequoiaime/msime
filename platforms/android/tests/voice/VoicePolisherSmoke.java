import app.msime.android.VoicePolisher;

/** Cancellation is checked before opening a network connection. */
public final class VoicePolisherSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        VoicePolisher cancelled = new VoicePolisher();
        cancelled.cancel();
        check(cancelled.polish("https://127.0.0.1:1/v1/chat/completions", "m", "token", "p", "你好") == null,
            "a cancelled polish does not start a request");
        System.out.println("Android voice polisher cancellation passed");
    }
}
