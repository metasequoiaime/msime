import app.msime.android.CandidatePreeditStylePolicy;
import app.msime.android.PhrasePreeditPolicy;

/** What 「候选栏预编辑」 removes from the strip, and the two things it must leave alone. */
public final class CandidatePreeditStylePolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(CandidatePreeditStylePolicy.style("empty").equals(CandidatePreeditStylePolicy.EMPTY),
            "empty is read as stored");
        check(CandidatePreeditStylePolicy.style("pinyin").equals(CandidatePreeditStylePolicy.PINYIN),
            "pinyin is read as stored");
        check(CandidatePreeditStylePolicy.style(null).equals(CandidatePreeditStylePolicy.PINYIN)
                && CandidatePreeditStylePolicy.style("").equals(CandidatePreeditStylePolicy.PINYIN)
                && CandidatePreeditStylePolicy.style("raw").equals(CandidatePreeditStylePolicy.PINYIN),
            "anything unrecognised reads as the shared default, which shows the composition");
        check(CandidatePreeditStylePolicy.showsComposition(CandidatePreeditStylePolicy.PINYIN)
                && !CandidatePreeditStylePolicy.showsComposition(CandidatePreeditStylePolicy.EMPTY),
            "only empty hides the composition");

        String pinyin = CandidatePreeditStylePolicy.PINYIN;
        String empty = CandidatePreeditStylePolicy.EMPTY;
        check("nihao".equals(CandidatePreeditStylePolicy.composedText(pinyin, "nihao", false)),
            "the spelling is shown by default");
        check(CandidatePreeditStylePolicy.composedText(empty, "nihao", false).isEmpty(),
            "the spelling is removed by 不显示");
        check("にほん".equals(CandidatePreeditStylePolicy.composedText(pinyin, "にほん", false)),
            "a Japanese reading is a composition like any other");
        check(CandidatePreeditStylePolicy.composedText(empty, "にほん", false).isEmpty(),
            "and is removed the same way");

        // A local mode's name says which mode is running. Removing it would leave the user in a
        // mode with nothing on screen saying so, which is not what hiding the spelling means.
        check("表情".equals(CandidatePreeditStylePolicy.composedText(empty, "表情", true)),
            "a local mode's own name survives 不显示");
        check("表情".equals(CandidatePreeditStylePolicy.composedText(pinyin, "表情", true)),
            "and is unaffected by the default");
        // What the mode spells past its trigger is preedit like any other.
        check(CandidatePreeditStylePolicy.composedText(empty, "Ksmile", false).isEmpty(),
            "text a local mode is spelling still follows the setting");

        check(CandidatePreeditStylePolicy.composedText(empty, null, false).isEmpty()
                && CandidatePreeditStylePolicy.composedText(pinyin, null, true).isEmpty(),
            "a missing title is the empty string rather than a crash");

        // The already-chosen part of a phrase is not preedit: the runtime is holding it out of the
        // document at this host's request, so hiding it would leave those characters nowhere.
        String hidden = CandidatePreeditStylePolicy.composedText(empty, "hao", false);
        check("你".equals(PhrasePreeditPolicy.title("你", hidden, false)),
            "the chosen part of the phrase stays on the strip when the spelling is hidden");
        check("你hao".equals(PhrasePreeditPolicy.title("你",
                CandidatePreeditStylePolicy.composedText(pinyin, "hao", false), false)),
            "and is drawn ahead of the spelling when it is shown");
        System.out.println("Android candidate preedit style policy passed");
    }
}
