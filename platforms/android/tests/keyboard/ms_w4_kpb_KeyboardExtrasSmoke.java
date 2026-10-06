package app.msime.android;

import java.util.List;

/** W4 键盘的纯逻辑：手写偏好与上文、单手侧栏、按键音音量、日志级别、键盘内语音的识别方式选择与调试行。 */
public final class ms_w4_kpb_KeyboardExtrasSmoke {
    public static void main(String[] arguments) {
        HandwritingPreferences defaults = HandwritingPreferences.defaults();
        check(defaults.mode() == HandwritingPreferences.Mode.OVERLAP, "overlap is the default mode");
        check(defaults.delayMillis() == 500 && defaults.showPinyin() && defaults.inkColor() == null
            && defaults.strokeWidth() == 3, "defaults follow the settings page");
        HandwritingPreferences line = HandwritingPreferences.of("line", 5000, false, "blue", 0);
        check(line.mode() == HandwritingPreferences.Mode.LINE, "line mode is read");
        check(line.delayMillis() == 1500 && line.strokeWidth() == 1, "out of range values are clamped");
        check(line.inkColor() == 0xFF1976D2, "blue ink");
        check(HandwritingPreferences.of("single", 100, true, "black", 9).inkColor() == 0xFF000000, "black ink");
        check(HandwritingPreferences.of("single", 100, true, "white", 9).inkColor() == 0xFFFFFFFF, "white ink");
        check(HandwritingPreferences.of("bogus", 800, true, "purple", 4).mode() == HandwritingPreferences.Mode.OVERLAP,
            "an unknown mode falls back to overlap");
        check(HandwritingPreferences.of("single", 800, true, "purple", 4).inkColor() == null,
            "an unknown colour follows the skin");
        check(HandwritingPreferences.of("single", 1200, true, "", 3).extraDelay(550) == 650, "a longer wait adds the difference");
        check(HandwritingPreferences.of("single", 200, true, "", 3).extraDelay(550) == 0, "a shorter wait cannot go below the service debounce");

        check(HandwritingRecognizer.clipPreContext(null).isEmpty(), "no context");
        check(HandwritingRecognizer.clipPreContext("今天\n天气").equals("今天天气"), "control characters are dropped");
        String longer = "一二三四五六七八九十甲乙丙丁戊己庚辛壬癸子丑寅卯";
        check(HandwritingRecognizer.clipPreContext(longer).equals(longer.substring(longer.length() - 20)),
            "the last twenty characters are kept");
        String emoji = HandwritingRecognizer.clipPreContext("😀".repeat(25));
        check(emoji.codePointCount(0, emoji.length()) == 20, "clipping counts code points");

        check(ImeFrame.oneHanded("left") && ImeFrame.oneHanded("right") && !ImeFrame.oneHanded("off")
            && !ImeFrame.oneHanded(null), "only left and right are one-handed");
        check(ImeFrame.gutterOnLeft("right") && !ImeFrame.gutterOnLeft("left"),
            "the gutter sits opposite the keyboard");
        check(OneHandGutterView.keysWidth(1000) == 850 && OneHandGutterView.gutterWidth(1000) == 150,
            "keys keep 85% of the width");

        check(ImeKeyFeedback.volumeFor(50) == .5f && ImeKeyFeedback.volumeFor(-3) == 0f
            && ImeKeyFeedback.volumeFor(400) == 1f, "volume is 0-100 mapped to 0-1");
        check(KeyPressAnimator.Style.fromPreference("ripple") == KeyPressAnimator.Style.RIPPLE
            && KeyPressAnimator.Style.fromPreference("unknown") == KeyPressAnimator.Style.NONE, "key animation styles");

        check(ImeLog.levelFor("error") == 6 && ImeLog.levelFor("warn") == 5 && ImeLog.levelFor("info") == 4
            && ImeLog.levelFor("debug") == 3 && ImeLog.levelFor("verbose") == 5 && ImeLog.levelFor(null) == 5,
            "log levels map to android.util.Log levels");
        ImeLog.applyLevel("debug");
        check(ImeLog.enabled(3), "debug passes at debug level");
        ImeLog.applyLevel("error");
        check(!ImeLog.enabled(5) && ImeLog.enabled(6), "error only lets errors through");
        ImeLog.applyLevel("warn");

        check(ImeVoiceEntry.choose(true, false, false, false, false) == ImeVoiceEntry.Engine.LOCAL,
            "a configured local model runs in the keyboard");
        check(ImeVoiceEntry.choose(false, true, true, true, true) == ImeVoiceEntry.Engine.STREAMING,
            "streaming runs in the keyboard while online");
        check(ImeVoiceEntry.choose(false, true, false, true, true) == ImeVoiceEntry.Engine.LOCAL,
            "offline with fallback and an installed model uses the local model");
        check(ImeVoiceEntry.choose(false, true, false, false, true) == ImeVoiceEntry.Engine.STREAMING,
            "without the fallback switch the configured engine stays");
        check(ImeVoiceEntry.choose(false, false, true, true, true) == null,
            "the system recogniser stays in its activity");
        check(ImeVoiceEntry.choose(false, false, false, true, false) == null,
            "no installed model, no fallback");

        check(ImeDebugOverlay.debugLine(12, 5, "").equals("调试 · 引擎 12 ms · 候选 5"), "debug line");
        check(ImeDebugOverlay.debugLine(-1, 0, "880").equals("调试 · 引擎 — · 候选 0 · 首选词频 880"),
            "debug line without a timing yet");

        check(List.of("，", "。", "？", "！").equals(NineKeyLayout.punctuation()), "nine-key punctuation");
        System.out.println("Android W4 keyboard extras passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
