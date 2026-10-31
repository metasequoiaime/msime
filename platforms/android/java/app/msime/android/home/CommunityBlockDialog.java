package app.msime.android.home;

import android.content.Context;
import android.widget.Button;
import androidx.appcompat.app.AlertDialog;
import androidx.fragment.app.Fragment;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.ScreenCaptureProtection;
import app.msime.android.ThemeColorPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;

/**
 * 屏蔽一位社区作者：先确认，再经 Rust 的 block 用登录账号发出；成功后由调用方关掉详情、刷新它来自的列表，服务端此后在所有社区目录里不再列出这位作者的作品，客户端不另做过滤。
 *
 * <p>屏蔽只认登录的水杉账号，不像举报那样退回本机匿名账号：目录按列表请求带的令牌过滤，匿名账号的屏蔽在重装后就没了，也不会作用在登录以后的列表上。所以没有登录时点「屏蔽作者」不弹确认框，先提示「登录水杉账号后才能屏蔽作者。」并打开登录面板，登录成功再接着确认。与 {@link CommunityReportDialog} 一样，完整列表、AI 页的回复模板和社区插件页共用这一处。
 */
final class CommunityBlockDialog {
    private CommunityBlockDialog() {}

    /**
     * 屏蔽作品 `itemId` 的作者。
     *
     * @param author 作品上写的作者名，确认框和提示里称呼他用；空串时叫「匿名作者」
     * @param kind 作品类型，与举报的 `kind` 相同（{@link CommunityRequest#BLOCK_KINDS}）
     * @param onBlocked 屏蔽成功、提示已经显示之后在主线程上调用：关掉详情、刷新列表
     */
    static void show(Fragment fragment, String author, String kind, String itemId, Runnable onBlocked) {
        String name = CommunityRequest.authorLabel(author);
        requireSignIn(fragment, () -> confirm(fragment, name, kind, itemId, onBlocked));
    }

    /**
     * 登录了水杉账号就在主线程上运行 `then`；没有登录就提示「登录水杉账号后才能屏蔽作者。」并打开登录面板，登录成功后再运行 `then`，取消或失败时不运行。读登录状态要等 Rust 的会话锁，放在网络线程上。
     */
    static void requireSignIn(Fragment fragment, Runnable then) {
        HostTask.runNetwork(fragment, context -> new CommunityCatalog(context).signedIn(), signedIn -> {
            if (fragment.getView() == null) return;
            if (Boolean.TRUE.equals(signedIn)) {
                then.run();
            } else {
                signIn(fragment, then);
            }
        });
    }

    private static void signIn(Fragment fragment, Runnable then) {
        MsToast.show(fragment.requireContext(), CommunityRequest.BLOCK_SIGN_IN);
        SignIn.start(fragment.requireActivity(), failure -> {
            if (fragment.getView() == null) return;
            if (failure == null || failure.isEmpty()) {
                then.run();
            } else if (!LoginSheet.CANCELLED.equals(failure)) {
                MsToast.show(fragment.requireContext(), failure);
            }
        });
    }

    private static void confirm(Fragment fragment, String name, String kind, String itemId, Runnable onBlocked) {
        Context context = fragment.requireContext();
        AlertDialog dialog = ScreenCaptureProtection.protect(new MaterialAlertDialogBuilder(context)
            .setTitle(CommunityRequest.blockTitle(name))
            .setMessage(CommunityRequest.BLOCK_CONFIRM_MESSAGE)
            .setNegativeButton("取消", null)
            .setPositiveButton("屏蔽", (ignored, which) -> block(fragment, name, kind, itemId, onBlocked))
            .create());
        // 「屏蔽」是破坏性操作，按钮用错误色，与「取消」区分开。
        dialog.setOnShowListener(ignored -> {
            Button action = dialog.getButton(AlertDialog.BUTTON_POSITIVE);
            if (action != null) action.setTextColor(ThemeColorPolicy.danger(context));
        });
        dialog.show();
    }

    private static void block(Fragment fragment, String name, String kind, String itemId, Runnable onBlocked) {
        HostTask.runNetwork(fragment, context -> new CommunityCatalog(context).block(kind, itemId), result -> {
            if (fragment.getView() == null) return;
            Context context = fragment.requireContext();
            if (result == null) {
                MsToast.show(context, CommunityRequest.blockMessage(null));
                return;
            }
            if (result.failed()) {
                // 登录在确认之后失效（令牌过期、在别处退出）：和没有登录时一样请人登录，登录后再确认一次。
                if (CommunityRequest.blockNeedsSignIn(result.code())) {
                    signIn(fragment, () -> confirm(fragment, name, kind, itemId, onBlocked));
                    return;
                }
                String message = CommunityRequest.blockMessage(result.code());
                if (!message.isEmpty()) MsToast.show(context, message);
                return;
            }
            // 提示里用服务端答的作者名：作品上的名字可能已经过时。
            String blocked = result.author().name().isEmpty() ? name : result.author().name();
            MsToast.show(context, CommunityRequest.blockedNotice(blocked));
            onBlocked.run();
        });
    }
}
