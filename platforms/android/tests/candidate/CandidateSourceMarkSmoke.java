import app.msime.android.CandidateSourceMark;
import app.msime.android.CandidateSourceMark.Glyph;

/** 云候选与 AI 候选的来源标记：图形、两个共享偏好、读屏说明和皮肤横幅字符；其他来源和占位行不加标记。 */
public final class CandidateSourceMarkSmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) {
        check(CandidateSourceMark.glyph(2, true, "robot") == Glyph.CLOUD, "cloud candidate gets the cloud glyph");
        check(CandidateSourceMark.glyph(2, false, "robot") == Glyph.NONE, "cloud toggle off hides only the glyph");
        check(CandidateSourceMark.spokenSuffix(2).equals("，云候选"), "cloud source still spoken with the glyph hidden");
        check(CandidateSourceMark.glyph(3, false, "robot") == Glyph.ROBOT, "cloud toggle leaves the AI mark alone");
        check(CandidateSourceMark.glyph(3, true, "whale") == Glyph.WHALE, "whale style");
        check(CandidateSourceMark.glyph(3, true, null) == Glyph.ROBOT, "missing style falls back to robot");
        check(CandidateSourceMark.glyph(3, true, "octopus") == Glyph.ROBOT, "unknown style falls back to robot");
        check(CandidateSourceMark.spokenSuffix(3).equals("，AI 候选"), "AI source spoken");
        // 0 词库、1 用户词、4 英文、11 整句模型、12 AI 占位、15 云占位、16 联系人：都不加来源标记。
        for (int source : new int[] {-1, 0, 1, 4, 11, 12, 15, 16}) {
            check(CandidateSourceMark.glyph(source, true, "whale") == Glyph.NONE, "source " + source + " unmarked");
            check(CandidateSourceMark.spokenSuffix(source).isEmpty(), "source " + source + " not spoken as cloud or AI");
            check(CandidateSourceMark.bannerBadge(source, true, "robot").isEmpty(), "source " + source + " banner unmarked");
        }
        check(CandidateSourceMark.bannerBadge(2, true, "robot").equals("☁️"), "banner cloud emoji");
        check(CandidateSourceMark.bannerBadge(2, false, "robot").isEmpty(), "banner follows the cloud toggle");
        check(CandidateSourceMark.bannerBadge(3, true, "robot").equals("🤖"), "banner robot emoji");
        check(CandidateSourceMark.bannerBadge(3, true, "whale").equals("🐳"), "banner whale emoji");
        check(CandidateSourceMark.aiStyle("whale").equals("whale") && CandidateSourceMark.aiStyle("").equals("robot"),
            "style normalised");
        System.out.println("Android candidate source mark passed");
    }
}
