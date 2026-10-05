package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import app.msime.android.AppleWebSignIn;

/**
 * Apple 网页登录的回调 `<applicationId>://auth/apple?grant=…`（或 `?error=…`）。
 *
 * <p>没有界面（`Theme.NoDisplay`），在 `onCreate` 里处理完就 `finish()`。只接受本应用刚发起、尚未完成、未过期的那次流程：先取出并删除 `msime_auth_pending`，没有或已过期就丢弃这次回调，所以恶意应用伪造的回调没有对应的 verifier，换不到会话，同一个 grant 也只会被处理一次。兑换在工作线程上进行，结果交给仍在等待的登录面板；登录成功不触发同步。
 */
public final class AuthRedirectActivity extends Activity {
    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        handle(getIntent());
        finish();
    }

    private void handle(Intent intent) {
        Uri uri = intent == null ? null : intent.getData();
        if (uri == null || !getPackageName().equals(uri.getScheme()) || !"auth".equals(uri.getHost())
                || !"/apple".equals(uri.getPath())) {
            return;
        }
        Context context = getApplicationContext();
        AppleWebSignIn.Pending pending = AppleWebSignIn.takePending(context);
        if (pending == null) return;
        String grant = uri.getQueryParameter("grant");
        String error = uri.getQueryParameter("error");
        if ((error == null || error.isEmpty()) && !AppleWebSignIn.validGrant(grant)) error = "invalid_grant";
        String failure = error;
        new Thread(() -> SignIn.completeApple(context, pending, grant, failure), "msime-apple-sign-in").start();
        // 浏览器里的落地页跳回来时，这个透明的 Activity 可能落在浏览器的任务里；把本应用已有的任务带回前台，登录面板就在那里等结果。
        Intent launch = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (launch != null) {
            launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED);
            startActivity(launch);
        }
    }
}
