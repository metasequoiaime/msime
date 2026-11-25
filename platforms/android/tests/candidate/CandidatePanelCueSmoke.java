import app.msime.android.CandidatePanelCue;
import java.util.List;

/** 展开面板上云/AI 候选提示的来源和读屏文字。 */
public final class CandidatePanelCueSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(CandidatePanelCue.sources(List.of()).isEmpty(), "没有在等的就没有来源");
        check(CandidatePanelCue.description(List.of()).isEmpty(), "没有来源不念");
        check(CandidatePanelCue.sources(List.of(3, 2, 3)).equals(List.of(2, 3)), "云在前、AI 在后，各一次");
        check(CandidatePanelCue.sources(List.of("x", 7)).isEmpty(), "别的标记不算");
        check(CandidatePanelCue.description(List.of(2)).equals("有新的云候选，点按刷新候选面板"), "云候选");
        check(CandidatePanelCue.description(List.of(3)).equals("有新的 AI 候选，点按刷新候选面板"), "AI 候选");
        check(CandidatePanelCue.description(List.of(2, 3)).contains("云候选和 AI 候选"), "两种都有");
        System.out.println("CandidatePanelCueSmoke: passed");
    }
}
