import app.msime.android.AndroidLocalSettings;
import app.msime.android.ClipboardLayoutPolicy;

/** #5642：剪贴板面板的单列 / 双列设置与它排出的行。 */
public final class ClipboardLayoutPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(ClipboardLayoutPolicy.columns(ClipboardLayoutPolicy.ONE_COLUMN) == 1, "one column");
        check(ClipboardLayoutPolicy.columns(ClipboardLayoutPolicy.TWO_COLUMNS) == 2, "two columns");
        check(ClipboardLayoutPolicy.columns(null) == 1 && ClipboardLayoutPolicy.columns("three") == 1,
            "an unknown value falls back to one column");

        AndroidLocalSettings.Snapshot defaults = AndroidLocalSettings.defaults();
        check(ClipboardLayoutPolicy.ONE_COLUMN.equals(defaults.choice(AndroidLocalSettings.CLIPBOARD_COLUMNS)),
            "the panel stays one column until the user asks for two");
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.CLIPBOARD_COLUMNS);
        check(!spec.synced, "the column setting is local only: the sync field table does not know it");
        check(ClipboardLayoutPolicy.TWO_COLUMNS.equals(spec.accept(ClipboardLayoutPolicy.TWO_COLUMNS)),
            "two columns is an accepted value");
        check(spec.accept("2") == null && spec.accept(Boolean.TRUE) == null, "anything else is refused");

        // 双列：第 0、1 条在第一行，第 2 条开第二行；五条排三行，最后一行只有一条。
        check(ClipboardLayoutPolicy.row(0, 2) == 0 && ClipboardLayoutPolicy.row(1, 2) == 0,
            "the first two entries share a row");
        check(ClipboardLayoutPolicy.row(2, 2) == 1, "the third entry starts the next row");
        check(ClipboardLayoutPolicy.rows(5, 2) == 3 && ClipboardLayoutPolicy.rows(4, 2) == 2,
            "rows round up for an odd count");
        check(ClipboardLayoutPolicy.rows(5, 1) == 5 && ClipboardLayoutPolicy.row(4, 1) == 4,
            "one column is one entry per row");
        check(ClipboardLayoutPolicy.rows(0, 2) == 0, "no entries, no rows");
        try {
            ClipboardLayoutPolicy.row(0, 0);
            throw new AssertionError("zero columns is refused");
        } catch (IllegalArgumentException expected) {
            // 列数至少为 1。
        }
        System.out.println("ClipboardLayoutPolicySmoke ok");
    }
}
