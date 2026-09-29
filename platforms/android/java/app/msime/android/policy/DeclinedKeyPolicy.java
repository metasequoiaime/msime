package app.msime.android;

/**
 * 引擎不收的键，宿主自己上屏之前该不该先把组合结束掉。
 *
 * <p>Android draws the preedit as a real composing region, so committing a declined mark straight
 * into it replaces the pinyin rather than following it: "nihao" then "@" left only "@". Apple's
 * `handleSymbol` resolves the same case with finish_composition — the leading candidate — so the
 * mark lands after 你好.
 *
 * <p>Only punctuation takes this route. A declined digit is a candidate key that found no chip on
 * the current page, and committing the composition for it would pick a candidate the user cannot
 * see; that case stays where it is.
 */
public final class DeclinedKeyPolicy {
    private DeclinedKeyPolicy() {}

    /** Whether a declined key must finish the open composition before the host commits it. */
    public static boolean finishesComposition(boolean punctuation, boolean composing) {
        return punctuation && composing;
    }
}
