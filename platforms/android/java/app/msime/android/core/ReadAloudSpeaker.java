package app.msime.android;

import android.content.Context;
import android.content.Intent;
import android.content.pm.ResolveInfo;
import android.os.Bundle;
import android.speech.tts.TextToSpeech;
import android.speech.tts.Voice;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.function.Consumer;

/**
 * 长按候选菜单的「朗读」和背单词卡片的喇叭：用系统的 {@link TextToSpeech} 读一个英文单词。只用本机、不用再下载的英文声音（{@link ReadAloudVoicePolicy}），没有时不读并说明原因。引擎第一次朗读时才连接，之后一直用它，{@link #shutdown()} 时断开。所有方法都在主线程调用，回调也在主线程。见 .agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md。
 */
public final class ReadAloudSpeaker {
    public static final String NO_ENGINE = "系统没有可用的语音引擎";
    public static final String NO_OFFLINE_VOICE = "系统没有可离线使用的英文朗读语音";
    public static final String FAILED = "无法朗读";
    private static final String UTTERANCE = "msime-read-aloud";

    private final Context context;
    private TextToSpeech engine;
    private boolean ready;
    /** 引擎还在连接时点下的那个词；连上以后读最后点的那一个。 */
    private String pending;
    private Consumer<String> pendingNotice;

    public ReadAloudSpeaker(Context context) {
        this.context = context.getApplicationContext() == null ? context : context.getApplicationContext();
    }

    /** 装没装语音引擎。Android 11 起要在清单的 {@code <queries>} 里声明 {@code TTS_SERVICE} 才看得见引擎。打开菜单或画卡片时调用。 */
    public static boolean engineInstalled(Context context) {
        List<ResolveInfo> services = context.getPackageManager()
            .queryIntentServices(new Intent(TextToSpeech.Engine.INTENT_ACTION_TTS_SERVICE), 0);
        return services != null && !services.isEmpty();
    }

    /** 读 {@code text}（{@code NativeClient.readAloudText} 答的英文）。读不出来时把给用户看的那句话交给 {@code notice}；读出来时不回调。新的一次打断还在读的上一次。 */
    public void speak(String text, Consumer<String> notice) {
        if (text == null || text.isEmpty()) return;
        if (engine != null && ready) {
            speakNow(text, notice);
            return;
        }
        pending = text;
        pendingNotice = notice;
        if (engine != null) return;
        engine = new TextToSpeech(context, status -> {
            if (status != TextToSpeech.SUCCESS) {
                Consumer<String> failed = pendingNotice;
                shutdown();
                if (failed != null) failed.accept(NO_ENGINE);
                return;
            }
            ready = true;
            String word = pending;
            Consumer<String> answer = pendingNotice;
            pending = null;
            pendingNotice = null;
            if (word != null) speakNow(word, answer);
        });
    }

    private void speakNow(String text, Consumer<String> notice) {
        Set<Voice> installed;
        try {
            installed = engine.getVoices();
        } catch (RuntimeException error) {
            installed = null;
        }
        List<Voice> voices = installed == null ? List.of() : new ArrayList<>(installed);
        List<ReadAloudVoicePolicy.Option> options = new ArrayList<>(voices.size());
        for (Voice voice : voices) {
            Set<String> features = voice.getFeatures();
            options.add(new ReadAloudVoicePolicy.Option(voice.getLocale(), voice.isNetworkConnectionRequired(),
                features != null && features.contains(TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED),
                voice.getQuality()));
        }
        int chosen = ReadAloudVoicePolicy.choose(options);
        if (chosen < 0) {
            if (notice != null) notice.accept(NO_OFFLINE_VOICE);
            return;
        }
        if (engine.setVoice(voices.get(chosen)) != TextToSpeech.SUCCESS
            || engine.speak(text, TextToSpeech.QUEUE_FLUSH, new Bundle(), UTTERANCE) != TextToSpeech.SUCCESS) {
            if (notice != null) notice.accept(FAILED);
        }
    }

    /** 断开引擎，正在读的也停下。键盘服务或背单词页销毁时调用；之后再 {@link #speak} 会重新连接。 */
    public void shutdown() {
        if (engine != null) {
            engine.stop();
            engine.shutdown();
        }
        engine = null;
        ready = false;
        pending = null;
        pendingNotice = null;
    }
}
