import app.msime.android.EditorContextSnapshot;

public final class EditorContextSnapshotSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        Object editor = new Object();
        EditorContextSnapshot snapshot = new EditorContextSnapshot(
            editor, 7, "before fixture", "selection fixture", "after fixture");
        check(snapshot.matches(editor, 7, "before fixture", "selection fixture", "after fixture"));
        check(!snapshot.matches(editor, 8, "before fixture", "selection fixture", "after fixture"));
        check(!snapshot.matches(new Object(), 7, "before fixture", "selection fixture", "after fixture"));
        check(!snapshot.matches(editor, 7, "changed fixture", "selection fixture", "after fixture"));
        check(!snapshot.matches(editor, 7, "before fixture", null, "after fixture"));
        System.out.println("Android editor context snapshot: identity and bounded context changes passed");
    }
}
