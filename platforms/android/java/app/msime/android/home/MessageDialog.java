package app.msime.android.home;

import app.msime.android.ScreenCaptureProtection;
import android.content.Context;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;

/** A settings result or notice with one acknowledgement action. */
final class MessageDialog {
    private MessageDialog() {}

    static void show(Context context, CharSequence title, CharSequence message) {
        ScreenCaptureProtection.show(new MaterialAlertDialogBuilder(context)
            .setTitle(title)
            .setMessage(message)
            .setPositiveButton("好", null)
            .create());
    }

    /** A confirmation with no side effect when dismissed or cancelled. */
    static void confirm(Context context, CharSequence title, CharSequence message,
            CharSequence actionLabel, Runnable action) {
        ScreenCaptureProtection.show(new MaterialAlertDialogBuilder(context)
            .setTitle(title)
            .setMessage(message)
            .setNegativeButton("取消", null)
            .setPositiveButton(actionLabel, (dialog, which) -> action.run())
            .create());
    }
}
