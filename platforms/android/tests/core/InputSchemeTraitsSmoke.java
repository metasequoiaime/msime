import app.msime.android.InputSchemeTraits;
import java.util.Set;
import java.util.function.IntPredicate;

/** The host's copy of the Engine's scheme predicates, scheme by scheme; scripts/test-scheme-traits-parity.py checks the same table against crates/engine/src/types.rs. */
public final class InputSchemeTraitsSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void table(String name, IntPredicate trait, Set<Integer> expected) {
        for (int scheme = -1; scheme <= 10; scheme++) {
            check(trait.test(scheme) == expected.contains(scheme), name + " for scheme " + scheme);
        }
    }

    public static void main(String[] args) {
        check(InputSchemeTraits.QUANPIN == 0 && InputSchemeTraits.SHUANGPIN == 1 && InputSchemeTraits.WUBI == 2
            && InputSchemeTraits.JAPANESE == 3 && InputSchemeTraits.KOREAN == 4 && InputSchemeTraits.CANTONESE == 5
            && InputSchemeTraits.ZHUYIN == 6 && InputSchemeTraits.VIETNAMESE == 7 && InputSchemeTraits.STROKE == 8,
            "the shared View.scheme ordinals");
        table("isChinese", InputSchemeTraits::isChinese, Set.of(0, 1, 2, 5, 6, 8));
        table("outputsTraditionalNatively", InputSchemeTraits::outputsTraditionalNatively, Set.of(5, 6, 8));
        table("scriptConversionApplies", InputSchemeTraits::scriptConversionApplies, Set.of(0, 1, 2));
        table("usesChinesePunctuation", InputSchemeTraits::usesChinesePunctuation, Set.of(0, 1, 2, 3, 5, 6, 8));
        table("widensFullWidth", InputSchemeTraits::widensFullWidth, Set.of(0, 1, 2, 3, 5, 6, 8));
        table("opensLocalModes", InputSchemeTraits::opensLocalModes, Set.of(0, 1));
        table("showsGlosses", InputSchemeTraits::showsGlosses, Set.of(0, 1, 2, 4));
        table("commitsOnBlur", InputSchemeTraits::commitsOnBlur, Set.of(4, 6, 7));
        table("drawsReading", InputSchemeTraits::drawsReading, Set.of(3, 4, 6, 8));
        table("hasOpenableCandidateList", InputSchemeTraits::hasOpenableCandidateList, Set.of(4, 6));
        table("cancelKeepsComposition", InputSchemeTraits::cancelKeepsComposition, Set.of(4, 6, 7));
        table("locksCaret", InputSchemeTraits::locksCaret, Set.of(4, 6, 7));
        table("known", InputSchemeTraits::known, Set.of(0, 1, 2, 3, 4, 5, 6, 7, 8));
        table("letterComposition", InputSchemeTraits::letterComposition, Set.of(4, 7));
        System.out.println("Android scheme traits: ordinals and the Engine predicate table passed");
    }
}
