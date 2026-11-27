package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 展开面板开着时，在等的云/AI 候选给出的提示：画哪几个来源标记、写什么、读屏念什么。
 *
 * <p>提示浮在面板上面，不挤动格子；点它在等的候选就用上（面板换成新一代、回到顶部），收起面板也一样。提示只说候选到了，不说和面板里的不同：云候选可能和已有的词重复，由共享内核在用的时候去重。
 */
public final class CandidatePanelCue {
    public static final String LABEL = "新候选 · 刷新";

    private CandidatePanelCue() {}

    /** 在等的标记里出现过的来源，云在前、AI 在后，各一次；别的标记不算。 */
    public static List<Integer> sources(List<?> tags) {
        List<Integer> sources = new ArrayList<>();
        for (int source : new int[] {CandidateSourceMark.CLOUD_SOURCE, CandidateSourceMark.AI_SOURCE})
            if (tags.contains(source)) sources.add(source);
        return sources;
    }

    /** 读屏念的说明；没有在等的来源时为空。 */
    public static String description(List<Integer> sources) {
        boolean cloud = sources.contains(CandidateSourceMark.CLOUD_SOURCE);
        boolean ai = sources.contains(CandidateSourceMark.AI_SOURCE);
        if (!cloud && !ai) return "";
        String what = cloud && ai ? "云候选和 AI 候选" : cloud ? "云候选" : " AI 候选";
        return "有新的" + what + "，点按刷新候选面板";
    }
}
