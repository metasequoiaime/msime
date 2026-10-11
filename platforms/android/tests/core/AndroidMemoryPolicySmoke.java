import android.content.ComponentCallbacks2;
import app.msime.android.AndroidMemoryPolicy;

@SuppressWarnings("deprecation")
public final class AndroidMemoryPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(!AndroidMemoryPolicy.shouldReleaseProcessCache(
                ComponentCallbacks2.TRIM_MEMORY_RUNNING_MODERATE),
                "moderate running pressure should keep the warm model");
        check(AndroidMemoryPolicy.shouldReleaseProcessCache(
                ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW),
                "running low pressure should release the process cache");
        check(!AndroidMemoryPolicy.shouldReleaseProcessCache(
                ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN),
                "hiding the IME should keep the warm model");
        check(AndroidMemoryPolicy.shouldReleaseProcessCache(
                ComponentCallbacks2.TRIM_MEMORY_BACKGROUND),
                "background pressure should release the process cache");
        check(AndroidMemoryPolicy.shouldReleaseProcessCache(
                ComponentCallbacks2.TRIM_MEMORY_COMPLETE),
                "complete pressure should release the process cache");
        System.out.println("AndroidMemoryPolicySmoke ok");
    }
}
