package app.msime.android.home;

import android.content.Context;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;

/** A settings result or notice with one acknowledgement action. */
final class MessageDialog {
    private MessageDialog() {}

    static void show(Context context, CharSequence title, CharSequence message) {
        new MaterialAlertDialogBuilder(context)
            .setTitle(title)
            .setMessage(message)
            .setPositiveButton("好", null)
            .show();
    }
}
