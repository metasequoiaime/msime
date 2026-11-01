package app.msime.android.home;

import android.text.InputType;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.CloudApi;
import app.msime.android.TypingUploadApi;
import java.time.ZoneId;
import java.util.List;

/**
 * 绑定 QQ 群助手：群友在水杉输入法的 QQ 群里 @机器人 发 /bind 拿到 8 位绑定码，在这里输入，把这个群里的自己绑到当前的水杉账号；绑定后在群里发 /stats 能看到打字数据（数据来自打字统计页的「上传到云端」）。
 *
 * <p>列出已有的绑定，点一行确认后解绑。服务端只存群和成员标识的摘要，所以一行只有绑定时间。说法与共享设置页的 `qq-link-card.tsx` 相同。只有真实账号能绑定，入口在「我的」页的「同步」组，没登录时那一行是灰的。
 */
public final class QqLinksPage extends ReloadingDetailPage {
    private long generation;

    @Override protected void onContentDestroyed() {
        generation++;
    }

    @Override protected void onContentReady() {
        showLoading();
    }

    @Override protected boolean shouldReloadOnContentReady() { return false; }

    private void showLoading() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        GroupCard.add(target, null).note("正在读取…");
    }

    @Override protected void reload() {
        long request = ++generation;
        showLoading();
        HostTask.runNetwork(this, context -> {
            try {
                return (Object) new TypingUploadApi(new CloudApi(context)).links();
            } catch (CloudApi.Failure failure) {
                return failure;
            }
        }, result -> {
            LinearLayout current = contentColumn();
            if (current == null || request != generation) return;
            current.removeAllViews();
            render(current, result instanceof List<?> links ? links : null,
                result instanceof CloudApi.Failure failure ? failure.signedOut() ? "登录水杉账号后才能绑定。"
                    : TypingUploadApi.generalMessage(failure) : null);
        });
    }

    private void render(LinearLayout column, @Nullable List<?> links, @Nullable String failure) {
        // 这一组没有组标题，怎样拿到绑定码的说明挂在「绑定码」这一行的「?」上。
        GroupCard intro = GroupCard.add(column, null);
        intro.button("绑定码", null, "输入", this::askCode)
            .help("在水杉输入法的 QQ 群里 @机器人 发 /bind，把收到的 8 位绑定码填在这里。绑定后发 /stats 能看到你的打字数据（先在「统计」页打开「上传到云端」）；机器人在群里的回复所有人都看得到。\n\n绑定码 10 分钟内有效，只能用一次。");
        if (failure != null) {
            GroupCard.add(column, null).note(failure);
            return;
        }
        if (links == null || links.isEmpty()) return;
        GroupCard group = GroupCard.add(column, "已绑定").withDividers(16);
        ZoneId zone = ZoneId.systemDefault();
        for (Object item : links) {
            if (!(item instanceof TypingUploadApi.Link link)) continue;
            String title = "绑定于 " + TypingUploadApi.displayTime(link.createdAt(), zone);
            group.nav(title, null, "解绑", () -> confirmRemove(link, title));
        }
        group.footer("解绑后，那个群里的 /stats 不再显示你的数据；在群里发 /unbind 效果相同。");
    }

    private void askCode() {
        InputDialog dialog = new InputDialog(requireContext(), "输入绑定码", "在 QQ 群里 @机器人 发 /bind 拿到的 8 位数字");
        dialog.addField("8 位数字", "", InputType.TYPE_CLASS_NUMBER);
        dialog.setValidator(values -> TypingUploadApi.normalizeCode(values.get(0)) != null);
        dialog.setPrimary("绑定", values -> redeem(values.get(0)));
        dialog.show();
    }

    private void redeem(String code) {
        long request = ++generation;
        HostTask.runNetwork(this, context -> {
            try {
                new TypingUploadApi(new CloudApi(context)).redeem(code);
                return "";
            } catch (CloudApi.Failure failure) {
                return TypingUploadApi.linkMessage(failure);
            }
        }, failure -> {
            if (request != generation) return;
            MsToast.show(requireContext(), failure == null ? "没有成功，请稍后再试。"
                : failure.isEmpty() ? "已绑定。在 QQ 群里 @机器人 发 /stats 就能看到你的打字数据。" : failure);
            reload();
        });
    }

    private void confirmRemove(TypingUploadApi.Link link, String title) {
        new OptionSheet(requireContext(), "解绑", title)
            .destructive("解绑", () -> remove(link))
            .show();
    }

    private void remove(TypingUploadApi.Link link) {
        long request = ++generation;
        HostTask.runNetwork(this, context -> {
            try {
                new TypingUploadApi(new CloudApi(context)).remove(link.id());
                return "";
            } catch (CloudApi.Failure failure) {
                return TypingUploadApi.generalMessage(failure);
            }
        }, failure -> {
            if (request != generation) return;
            MsToast.show(requireContext(), failure == null ? "没有成功，请稍后再试。" : failure.isEmpty() ? "已解绑。" : failure);
            reload();
        });
    }
}
