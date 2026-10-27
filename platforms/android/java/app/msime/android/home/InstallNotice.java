package app.msime.android.home;

import app.msime.android.policy.InstallNoticePolicy;
import android.content.Context;
import android.content.pm.PackageInfo;
import android.os.Handler;
import android.os.Looper;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;

/** 首次安装或升级后的临时说明，只对每个安装包显示一次。 */
final class InstallNotice {
    private static final String STORE = "msime_install_notice_v1";
    private static final String SHOWN_INSTALL_STAMP = "shown_install_stamp";
    private static final String TITLE = "临时说明（首次运行）";
    private static final String MESSAGE = "特殊说明：这是临时提示，程序稳定后会删除。\n\n"
        + "如果升级后发现问题仍然存在，可以先完全退出并重新打开应用；必要时切换一次输入方案（例如全拼九键和全拼 26 键）再试。\n\n"
        + "首次安装后，词库和资源可能仍在准备中，请稍等片刻再使用键盘。";

    private InstallNotice() {}

    static void showIfNeeded(AppCompatActivity activity) {
        if (activity.isFinishing() || activity.isDestroyed()) return;
        String stamp = installStamp(activity);
        if (!InstallNoticePolicy.shouldShow(stamp,
                activity.getSharedPreferences(STORE, Context.MODE_PRIVATE)
                    .getString(SHOWN_INSTALL_STAMP, null))) return;
        activity.getSharedPreferences(STORE, Context.MODE_PRIVATE).edit()
            .putString(SHOWN_INSTALL_STAMP, stamp).apply();
        var dialog = new MaterialAlertDialogBuilder(activity)
            .setTitle(TITLE)
            .setMessage(MESSAGE)
            .setPositiveButton("知道了", null)
            .create();
        dialog.setOnShowListener(ignored -> new Handler(Looper.getMainLooper())
            .postDelayed(() -> { if (dialog.isShowing()) dialog.dismiss(); },
                InstallNoticePolicy.dismissDelayMillis(MESSAGE.length())));
        dialog.show();
    }

    @Nullable
    private static String installStamp(Context context) {
        try {
            PackageInfo info = context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0);
            return info.lastUpdateTime > 0 ? Long.toString(info.lastUpdateTime) : null;
        } catch (Exception missing) {
            return null;
        }
    }
}
