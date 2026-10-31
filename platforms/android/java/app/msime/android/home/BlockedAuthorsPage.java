package app.msime.android.home;

import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import java.time.ZoneId;
import java.util.ArrayList;
import java.util.List;

/**
 * 已屏蔽的作者：登录账号在社区里屏蔽的作者，每位一行（作者名、屏蔽日期 yyyy-MM-dd），行尾「取消屏蔽」。从「我的 › 隐私」的「社区」组进来。
 *
 * <p>名单和取消屏蔽经 Rust 的 block_list、unblock（{@link CommunityCatalog#blocks}、{@link CommunityCatalog#unblock}），只用登录的水杉账号；没有登录时这一页只说「登录水杉账号后才能屏蔽作者。」并给登录入口，登录后重读。屏蔽本身在社区作品的详情里（{@link CommunityBlockDialog}）。取消屏蔽后服务端在下一次列表请求里重新列出这位作者的作品，这里不碰本机的任何列表。
 */
public final class BlockedAuthorsPage extends ReloadingDetailPage {
    private long generation;
    /** 正在取消屏蔽的那一条；非空时所有「取消屏蔽」按钮禁用，避免连点发出重复请求。 */
    @Nullable private String unblocking;
    /** 当前列表里每位作者那一行的「取消屏蔽」按钮所在的行。 */
    private final List<GroupCard.Row> rows = new ArrayList<>();

    @Override protected void onContentDestroyed() {
        generation++;
        unblocking = null;
        rows.clear();
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
        unblocking = null;
        rows.clear();
        showLoading();
        HostTask.runNetwork(this, context -> {
            CommunityCatalog catalog = new CommunityCatalog(context);
            // 没有登录时不发请求，直接显示登录入口。
            return catalog.signedIn() ? catalog.blocks() : new CommunityCatalog.Blocks(null, "community_unauthorized");
        }, result -> {
            LinearLayout current = contentColumn();
            if (current == null || request != generation) return;
            current.removeAllViews();
            if (result == null) {
                failure(current, CommunityRequest.blockListMessage(null));
                return;
            }
            if (result.failed()) {
                if (CommunityRequest.blockNeedsSignIn(result.code())) {
                    signedOut(current);
                } else {
                    // 读的期间登录换了号（community_cancelled）也落在这里：说「登录已切换，请重试。」，由人点重试，不自动重读。
                    failure(current, CommunityRequest.rustMessage(result.code()));
                }
                return;
            }
            render(current, result.items());
        });
    }

    private void signedOut(LinearLayout column) {
        GroupCard card = GroupCard.add(column, null);
        card.note(CommunityRequest.BLOCK_SIGN_IN);
        card.button("登录水杉账号", null, "登录", this::signIn);
    }

    private void failure(LinearLayout column, String message) {
        GroupCard card = GroupCard.add(column, null);
        card.note(message);
        card.button("重新读取", null, "重试", this::reload);
    }

    private void render(LinearLayout column, List<CommunityRequest.BlockedAuthor> authors) {
        if (authors.isEmpty()) {
            GroupCard.add(column, null).note(CommunityRequest.BLOCKED_AUTHORS_EMPTY);
            return;
        }
        GroupCard group = GroupCard.add(column, null).withDividers(16);
        ZoneId zone = ZoneId.systemDefault();
        for (CommunityRequest.BlockedAuthor author : authors) {
            String date = CommunityRequest.blockedDate(author.createdAt(), zone);
            rows.add(group.button(CommunityRequest.authorLabel(author.name()), date.isEmpty() ? null : date,
                "取消屏蔽", () -> unblock(author)));
        }
    }

    private void unblock(CommunityRequest.BlockedAuthor author) {
        if (unblocking != null) return;
        unblocking = author.id();
        long request = generation;
        setButtonsEnabled(false);
        HostTask.runNetwork(this, context -> new CommunityCatalog(context).unblock(author.id()), code -> {
            if (contentColumn() == null || request != generation) return;
            unblocking = null;
            // 成功时重读名单，这一行就不在了；登录失效或换了号时也重读，按现在的账号显示登录入口或它的名单。
            if (code != null && (code.isEmpty() || CommunityRequest.blockNeedsSignIn(code)
                    || "community_cancelled".equals(code))) {
                reload();
                return;
            }
            MsToast.show(requireContext(), CommunityRequest.blockListMessage(code));
            setButtonsEnabled(true);
        });
    }

    /** 列表里的「取消屏蔽」按钮全部启用或禁用。 */
    private void setButtonsEnabled(boolean enabled) {
        for (GroupCard.Row row : rows) {
            if (row.button() != null) row.button().setEnabled(enabled);
        }
    }

    private void signIn() {
        SignIn.start(requireActivity(), failure -> {
            if (getView() == null) return;
            if (failure != null && !failure.isEmpty() && !LoginSheet.CANCELLED.equals(failure)) {
                MsToast.show(requireContext(), failure);
            }
            reload();
        });
    }
}
