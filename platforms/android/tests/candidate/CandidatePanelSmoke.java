import app.msime.android.CandidateWrapPolicy;
import java.util.Arrays;

public final class CandidatePanelSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    static void rejects(Runnable operation) {
        try {
            operation.run();
            throw new AssertionError("Expected invalid candidate dimensions to be rejected");
        } catch (IllegalArgumentException expected) {
            // Expected.
        }
    }

    public static void main(String[] args) {
        check(Arrays.equals(CandidateWrapPolicy.rows(100, 10, new int[] {40, 40, 30, 100, 1}),
            new int[] {0, 0, 1, 2, 3}));
        check(Arrays.equals(CandidateWrapPolicy.rows(100, 10, new int[] {40, 50}),
            new int[] {0, 0}));
        check(Arrays.equals(CandidateWrapPolicy.rows(100, 10, new int[] {120, 20}),
            new int[] {0, 1}));
        check(CandidateWrapPolicy.rows(100, 10, new int[0]).length == 0);
        rejects(() -> CandidateWrapPolicy.rows(-1, 0, new int[] {1}));
        rejects(() -> CandidateWrapPolicy.rows(1, -1, new int[] {1}));
        rejects(() -> CandidateWrapPolicy.rows(1, 0, new int[] {-1}));
        System.out.println("Android candidate panel: measured-width wrapping and bounds passed");
    }
}
