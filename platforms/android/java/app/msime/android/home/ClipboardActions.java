package app.msime.android.home;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;

/** Shared copy action for settings pages that confirm a successful clipboard write. */
final class ClipboardActions {
    private ClipboardActions() {}

    static void copyText(Context context, String label, String text, String confirmation) {
        ClipboardManager clipboard = context.getSystemService(ClipboardManager.class);
        if (clipboard == null) return;
        clipboard.setPrimaryClip(ClipData.newPlainText(label, text));
        MsToast.show(context, confirmation);
    }
}
