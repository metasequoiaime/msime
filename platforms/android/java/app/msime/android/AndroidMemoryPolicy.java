package app.msime.android;

import android.content.ComponentCallbacks2;

/** Decides when the IME should give back process-level model caches. */
@SuppressWarnings("deprecation")
public final class AndroidMemoryPolicy {
    private AndroidMemoryPolicy() {}

    /**
     * Keep the Japanese warm cache when the IME window is merely hidden, but release it when
     * Android reports running-process pressure or moves the process onto the background LRU list.
     * The running range is bounded explicitly so {@code TRIM_MEMORY_UI_HIDDEN} does not flush the
     * warm cache after every input field change; Android may add intermediate levels within each
     * range.
     */
    @SuppressWarnings("deprecation")
    public static boolean shouldReleaseProcessCache(int level) {
        return (level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW
                    && level < ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN)
            || level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND;
    }
}
