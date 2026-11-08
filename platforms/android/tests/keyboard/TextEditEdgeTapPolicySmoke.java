import app.msime.android.TextEditEdgeTapPolicy;
import app.msime.android.TextEditPanelModel.Action;

public final class TextEditEdgeTapPolicySmoke {
    public static void main(String[] args) {
        TextEditEdgeTapPolicy edge = new TextEditEdgeTapPolicy();
        check(TextEditEdgeTapPolicy.DOUBLE_TAP_MS == 400, "the window is 400 ms, measured release to release");

        check(edge.tap(Action.DOCUMENT_END, 1_000) == Action.LINE_END, "a tap on ⇥ goes to the line end at once");
        check(edge.tap(Action.DOCUMENT_END, 1_400) == Action.DOCUMENT_END, "a second ⇥ at exactly 400 ms goes to the end");
        check(edge.tap(Action.DOCUMENT_END, 1_450) == Action.LINE_END, "the double tap is used up: a third tap starts over");

        check(edge.tap(Action.DOCUMENT_START, 2_000) == Action.LINE_START, "a tap on ⇤ goes to the line start");
        check(edge.tap(Action.DOCUMENT_START, 2_371) == Action.DOCUMENT_START,
            "a relaxed double tap (371 ms, the slowest measured) still goes to the start");

        check(edge.tap(Action.DOCUMENT_START, 3_000) == Action.LINE_START, "first tap");
        check(edge.tap(Action.DOCUMENT_START, 3_401) == Action.LINE_START, "a second tap after 400 ms counts as a single tap");
        check(edge.tap(Action.DOCUMENT_START, 3_500) == Action.DOCUMENT_START, "and pairs with the next one in time");

        check(edge.tap(Action.DOCUMENT_START, 4_000) == Action.LINE_START, "⇤");
        check(edge.tap(Action.DOCUMENT_END, 4_100) == Action.LINE_END, "⇥ right after ⇤ is a single tap");
        check(edge.tap(Action.DOCUMENT_START, 4_200) == Action.LINE_START, "and so is ⇤ right after ⇥");

        check(edge.tap(Action.DOCUMENT_END, 5_000) == Action.LINE_END, "first tap");
        edge.reset();
        check(edge.tap(Action.DOCUMENT_END, 5_100) == Action.LINE_END, "reset forgets the first tap");

        check(edge.tap(Action.DOCUMENT_END, 6_000) == Action.LINE_END, "first tap");
        check(edge.tap(Action.COPY, 6_050) == Action.COPY, "other keys pass through");
        check(edge.tap(Action.DOCUMENT_END, 6_100) == Action.LINE_END, "and break the double tap");

        check(edge.tap(Action.DOCUMENT_END, 7_000) == Action.LINE_END, "first tap");
        check(edge.tap(Action.DOCUMENT_END, 6_900) == Action.LINE_END, "a clock going backwards is not a double tap");
        System.out.println("Android text edit ⇤ ⇥: single tap to line, 400 ms double tap to document passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
