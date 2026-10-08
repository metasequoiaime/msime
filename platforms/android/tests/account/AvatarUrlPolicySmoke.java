import app.msime.android.DeviceDataApi;

public final class AvatarUrlPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        for (String url : new String[] {
            "https://media.msime.app/avatars/fixture.jpg",
            "https://lh3.googleusercontent.com/a/fixture=s96-c",
            "https://googleusercontent.com/a/fixture"
        }) check(DeviceDataApi.avatarUrlAllowed(url), "accepted avatar URL: " + url);
        for (String url : new String[] {
            "http://media.msime.app/avatars/fixture.jpg",
            "https://media.msime.app:8443/avatars/fixture.jpg",
            "https://user@media.msime.app/avatars/fixture.jpg",
            "https://evilgoogleusercontent.com/a/fixture",
            "https://example.test/fixture.jpg",
            "not a URL"
        }) check(!DeviceDataApi.avatarUrlAllowed(url), "rejected avatar URL: " + url);
        System.out.println("Android avatar URL policy passed");
    }
}
