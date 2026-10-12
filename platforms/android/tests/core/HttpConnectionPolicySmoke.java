import app.msime.android.HttpConnectionPolicy;

public final class HttpConnectionPolicySmoke {
    public static void main(String[] args) {
        if (HttpConnectionPolicy.isSuccessful(199)
                || !HttpConnectionPolicy.isSuccessful(200)
                || !HttpConnectionPolicy.isSuccessful(299)
                || HttpConnectionPolicy.isSuccessful(300)
                || HttpConnectionPolicy.isSuccessful(-1)) {
            throw new AssertionError("HTTP success status must be exactly 2xx");
        }
        System.out.println("Android HTTP connection policy passed");
    }
}
