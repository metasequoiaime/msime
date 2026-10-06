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
 * <p>没有界面（`Theme.NoDisplay`），在 `onCreate` 里处理完就 `finish()`。先只看回调地址的形状（{@link AppleWebSignIn#acceptableCallback}），不对就丢弃、不碰 `msime_auth_pending`；再看有没有本应用刚发起、未过期的那次流程，没有也丢弃。恶意应用伪造的回调没有对应的 verifier，换不到会话；服务端以「grant 无效」拒绝时等待中的流程原样保留，真正的回调随后仍能完成（见 {@link SignIn#completeApple}）。兑换在工作线程上串行进行，结果交给仍在等待的登录面板；登录成功不触发同步。
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
        String grant = uri.getQueryParameter("grant");
        String error = uri.getQueryParameter("error");
        // 形状不对的回调不碰等待中的流程，免得任何应用随手发一个 VIEW 就打断正在进行的登录。
        if (!AppleWebSignIn.acceptableCallback(grant, error)) return;
        Context context = getApplicationContext();
        if (AppleWebSignIn.peekPending(context) == null) return;
        new Thread(() -> SignIn.completeApple(context, grant, error), "msime-apple-sign-in").start();
        // 浏览器里的落地页跳回来时，这个透明的 Activity 可能落在浏览器的任务里；把本应用已有的任务带回前台，登录面板就在那里等结果。
        Intent launch = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (launch != null) {
            launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED);
            startActivity(launch);
        }
    }
}
