package app.msime.android;

import java.util.List;
import java.util.Locale;

/**
 * 「朗读」挑哪一个系统声音。只挑语言是英文、不需要网络、也不用再下载数据的声音：{@code Voice.isNetworkConnectionRequired()} 为真的声音会把词发给引擎的服务器，带着 {@code TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED} 的声音在 {@code setVoice} 时会触发下载，两种都不用。先美式英语，再别的英文；同一档里音质高的在前，音质一样时按引擎列出的顺序。一个都没有时不读，见 .agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md。
 *
 * 规则写成纯逻辑，JVM 冒烟测得到；{@code ReadAloudSpeaker} 把引擎给的 {@code Voice} 换成 {@link Option} 再交进来。
 */
public final class ReadAloudVoicePolicy {
    private ReadAloudVoicePolicy() {}

    /** 一个系统声音里挑选时用得到的几项。 */
    public static final class Option {
        final Locale locale;
        final boolean network;
        final boolean notInstalled;
        final int quality;

        public Option(Locale locale, boolean network, boolean notInstalled, int quality) {
            this.locale = locale;
            this.network = network;
            this.notInstalled = notInstalled;
            this.quality = quality;
        }
    }

    /** 选中的那个在 {@code options} 里的下标；没有能用的时为 -1。 */
    public static int choose(List<Option> options) {
        int best = -1;
        int bestRank = Integer.MIN_VALUE;
        for (int index = 0; index < options.size(); index++) {
            Option option = options.get(index);
            if (option == null || option.locale == null || option.network || option.notInstalled
                || !"en".equals(option.locale.getLanguage())) continue;
            // 美式英语整整高一档，音质只在同一档里比。
            int rank = ("US".equals(option.locale.getCountry()) ? 1_000_000 : 0) + option.quality;
            if (rank > bestRank) {
                best = index;
                bestRank = rank;
            }
        }
        return best;
    }
}
