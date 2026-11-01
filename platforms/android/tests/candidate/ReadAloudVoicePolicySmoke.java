import app.msime.android.ReadAloudVoicePolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class ReadAloudVoicePolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    static ReadAloudVoicePolicy.Option voice(Locale locale, boolean network, boolean notInstalled, int quality) {
        return new ReadAloudVoicePolicy.Option(locale, network, notInstalled, quality);
    }

    public static void main(String[] args) {
        // 联网合成和还要下载的声音从不选，哪怕音质更高、哪怕是美式英语。
        List<ReadAloudVoicePolicy.Option> voices = new ArrayList<>(List.of(
            voice(Locale.US, true, false, 500),
            voice(Locale.US, false, true, 500),
            voice(Locale.CHINA, false, false, 500),
            voice(Locale.UK, false, false, 300)));
        check(ReadAloudVoicePolicy.choose(voices) == 3);
        // 本机的美式英语比别的英文高一档，同一档里音质高的在前，一样时取先列出的。
        voices.add(voice(Locale.US, false, false, 200));
        check(ReadAloudVoicePolicy.choose(voices) == 4);
        voices.add(voice(Locale.US, false, false, 400));
        voices.add(voice(Locale.US, false, false, 400));
        check(ReadAloudVoicePolicy.choose(voices) == 5);
        check(ReadAloudVoicePolicy.choose(List.of(voice(Locale.ENGLISH, false, false, 100))) == 0);
        // 一个能用的都没有时不读。
        check(ReadAloudVoicePolicy.choose(List.of()) == -1);
        check(ReadAloudVoicePolicy.choose(List.of(voice(Locale.US, true, false, 500), voice(Locale.JAPAN, false, false, 300))) == -1);
        List<ReadAloudVoicePolicy.Option> withNull = new ArrayList<>();
        withNull.add(null);
        withNull.add(voice(null, false, false, 300));
        check(ReadAloudVoicePolicy.choose(withNull) == -1);
        System.out.println("Android read aloud voice: only offline installed English voices, US first, quality within a tier passed");
    }
}
