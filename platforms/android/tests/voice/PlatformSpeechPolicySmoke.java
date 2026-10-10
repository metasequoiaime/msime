import android.speech.SpeechRecognizer;
import app.msime.android.PlatformSpeechPolicy;

/** 系统语音识别服务的错误提示、转交识别窗口的条件与音量换算（#5553）。 */
public final class PlatformSpeechPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static boolean fits(String message) {
        return message.codePointCount(0, message.length()) <= PlatformSpeechPolicy.MAX_LENGTH;
    }

    public static void main(String[] args) {
        // 每个错误码的提示都不同于旧的那句「语音识别未返回结果」，并且带着错误码，反馈时能对上号。
        int[] errors = {
            SpeechRecognizer.ERROR_NETWORK_TIMEOUT, SpeechRecognizer.ERROR_NETWORK,
            SpeechRecognizer.ERROR_AUDIO, SpeechRecognizer.ERROR_SERVER, SpeechRecognizer.ERROR_CLIENT,
            SpeechRecognizer.ERROR_SPEECH_TIMEOUT, SpeechRecognizer.ERROR_NO_MATCH,
            SpeechRecognizer.ERROR_RECOGNIZER_BUSY, SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS,
            SpeechRecognizer.ERROR_TOO_MANY_REQUESTS, SpeechRecognizer.ERROR_SERVER_DISCONNECTED,
            SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED, SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE,
            SpeechRecognizer.ERROR_CANNOT_CHECK_SUPPORT, 99,
        };
        for (int error : errors) {
            String message = PlatformSpeechPolicy.message(error, null);
            String code = "（错误码 " + error + "）";
            check(message.contains(code), "message carries error " + error);
            // Toast 放不下时截掉的是结尾，错误码必须排在出路提示前面。
            check(message.endsWith(code) || message.endsWith(code + "；可改用本地模型或豆包"),
                "the error code comes before the way out for " + error);
            // Android 12 起文字 Toast 只显示两行，超出的直接截掉；以前截掉的正是该做的事和错误码。
            check(fits(message), "message fits two toast lines for " + error + ": " + message);
            check(!message.startsWith("语音识别未返回结果"), "message names a reason for " + error);
        }
        check(PlatformSpeechPolicy.message(SpeechRecognizer.ERROR_NETWORK, null).contains("网络"), "network errors say network");
        String network = PlatformSpeechPolicy.message(SpeechRecognizer.ERROR_NETWORK, null);
        check(PlatformSpeechPolicy.message(SpeechRecognizer.ERROR_NETWORK_TIMEOUT, null)
                .startsWith(network.substring(0, network.indexOf("（错误码"))),
            "both network errors share one explanation");
        // 缺权限的是识别服务应用（水杉自己的权限在调起前已经查过），提示点名该给哪个应用开麦克风；名字取不到、太长或带控制字符时退回泛称，整句仍放得进两行。
        int denied = SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS;
        check(PlatformSpeechPolicy.message(denied, null).equals("请给默认语音识别应用开启麦克风权限（错误码 9）"),
            "a permission error without the service name asks for the default recognizer app");
        check(PlatformSpeechPolicy.message(denied, " 讯飞语音 ").equals("请给「讯飞语音」开启麦克风权限（错误码 9）"),
            "a permission error names the recognizer app");
        String longest = "一二三四五六七八九十一二";
        check(PlatformSpeechPolicy.message(denied, longest).contains("「" + longest + "」")
                && fits(PlatformSpeechPolicy.message(denied, longest)),
            "the longest accepted name still fits two toast lines");
        for (String unusable : new String[] {"", "   ", longest + "三", "Speech\nService"}) {
            check(PlatformSpeechPolicy.message(denied, unusable).contains("默认语音识别应用"),
                "an unusable name falls back to the default recognizer app: " + unusable);
        }
        check(PlatformSpeechPolicy.message(SpeechRecognizer.ERROR_NETWORK, "讯飞语音")
                .equals(PlatformSpeechPolicy.message(SpeechRecognizer.ERROR_NETWORK, null)),
            "only the permission error names the recognizer app");
        check(PlatformSpeechPolicy.message(SpeechRecognizer.ERROR_SPEECH_TIMEOUT, null).contains("没有听到声音"),
            "a speech timeout says nothing was heard");
        check(PlatformSpeechPolicy.message(99, null).startsWith("系统语音识别失败"), "an unknown code still says what failed");

        // 设备识别服务自身的故障要给出路（#5553 这台设备上系统识别一直失败，只报错误码等于没有办法）；用户这边能解决的不给。
        for (int error : new int[] {SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT,
                SpeechRecognizer.ERROR_SERVER, SpeechRecognizer.ERROR_SERVER_DISCONNECTED, SpeechRecognizer.ERROR_CLIENT,
                SpeechRecognizer.ERROR_TOO_MANY_REQUESTS, SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED,
                SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE, SpeechRecognizer.ERROR_CANNOT_CHECK_SUPPORT, 99}) {
            String message = PlatformSpeechPolicy.message(error, null);
            check(message.contains("本地模型") && message.contains("豆包"), "a service fault offers another engine for " + error);
        }
        for (int error : new int[] {SpeechRecognizer.ERROR_SPEECH_TIMEOUT, SpeechRecognizer.ERROR_NO_MATCH,
                SpeechRecognizer.ERROR_AUDIO, SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS,
                SpeechRecognizer.ERROR_RECOGNIZER_BUSY}) {
            check(!PlatformSpeechPolicy.message(error, null).contains("豆包"), "a user-side failure is not sent to settings for " + error);
        }

        // 服务报了成功却没有文字时没有错误码可报，也不能冒用「错误码 7」，否则反馈回来的错误码分不清是哪种失败。
        String empty = PlatformSpeechPolicy.emptyResult();
        check(!empty.contains("错误码"), "an empty result carries no borrowed error code");
        check(!empty.equals(PlatformSpeechPolicy.message(SpeechRecognizer.ERROR_NO_MATCH, null)), "an empty result is not a no-match error");
        check(empty.contains("没有返回文字") && empty.contains("本地模型"), "an empty result says so and offers another engine");
        check(fits(empty), "an empty result fits two toast lines");

        // 只有还没开始聆听、并且是调用方一侧被拒时才转交识别窗口；已经在听之后的失败直接提示。
        check(PlatformSpeechPolicy.retryInActivity(SpeechRecognizer.ERROR_CLIENT, false),
            "a client error before listening retries in the activity");
        check(PlatformSpeechPolicy.retryInActivity(SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS, false),
            "a permission refusal before listening retries in the activity");
        check(PlatformSpeechPolicy.retryInActivity(SpeechRecognizer.ERROR_SERVER_DISCONNECTED, false),
            "a dropped service connection before listening retries in the activity");
        check(!PlatformSpeechPolicy.retryInActivity(SpeechRecognizer.ERROR_CLIENT, true),
            "once listening, the activity would fail the same way");
        check(!PlatformSpeechPolicy.retryInActivity(SpeechRecognizer.ERROR_NETWORK, false)
                && !PlatformSpeechPolicy.retryInActivity(SpeechRecognizer.ERROR_NO_MATCH, false)
                && !PlatformSpeechPolicy.retryInActivity(SpeechRecognizer.ERROR_SPEECH_TIMEOUT, false),
            "network and no-speech errors are reported, not retried");

        check(PlatformSpeechPolicy.level(-10f) == 0f && PlatformSpeechPolicy.level(Float.NaN) == 0f,
            "quiet or missing values are silence");
        check(PlatformSpeechPolicy.level(4f) == 0.5f, "the middle of the range is half");
        check(PlatformSpeechPolicy.level(30f) == 1f, "loud values are capped");
        check(PlatformSpeechPolicy.smoothed(0.2f, 0.9f) == 0.9f, "rising volume is followed at once");
        float falling = PlatformSpeechPolicy.smoothed(0.8f, 0f);
        check(falling > 0f && falling < 0.8f, "falling volume decays rather than dropping");
        check(PlatformSpeechPolicy.smoothed(0.5f, Float.NaN) < 0.5f && PlatformSpeechPolicy.smoothed(0f, 5f) == 1f,
            "the shown level stays within 0-1");
        System.out.println("Android platform speech policy passed");
    }
}
