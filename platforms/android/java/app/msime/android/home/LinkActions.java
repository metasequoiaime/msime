package app.msime.android.home;

import android.content.Context;
import android.content.Intent;
import android.net.Uri;

/** Shared intents for opening links outside the settings UI. */
final class LinkActions {
    private LinkActions() {}

    /** Returns false when the device has no activity able to handle the URI. */
    static boolean open(Context context, String url) {
        try {
            context.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
            return true;
        } catch (RuntimeException unavailable) {
            return false;
        }
    }

    /** Opens a web link with the browser category requested by policy pages. */
    static boolean openBrowsable(Context context, String url) {
        try {
            context.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url))
                .addCategory(Intent.CATEGORY_BROWSABLE));
            return true;
        } catch (RuntimeException unavailable) {
            return false;
        }
    }

    /** Opens only the schemes supported by rendered Markdown links. */
    static boolean openMarkdown(Context context, String url) {
        Uri uri = Uri.parse(url);
        String scheme = uri.getScheme();
        if (scheme == null || !(scheme.equalsIgnoreCase("http")
                || scheme.equalsIgnoreCase("https") || scheme.equalsIgnoreCase("mailto"))) return false;
        return open(context, url);
    }
}
