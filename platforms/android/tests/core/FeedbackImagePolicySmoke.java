import app.msime.android.FeedbackImagePolicy;
import java.io.ByteArrayInputStream;

public final class FeedbackImagePolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) throws Exception {
        byte[] small = new byte[FeedbackImagePolicy.MAX_SOURCE_BYTES];
        small[0] = 7;
        byte[] copied = FeedbackImagePolicy.readSource(new ByteArrayInputStream(small));
        check(copied != null && copied.length == small.length && copied[0] == 7);
        check(FeedbackImagePolicy.readSource(new ByteArrayInputStream(
            new byte[FeedbackImagePolicy.MAX_SOURCE_BYTES + 1])) == null);
        check(FeedbackImagePolicy.readSource(null) == null);
        System.out.println("Android feedback image source policy passed");
    }
}
