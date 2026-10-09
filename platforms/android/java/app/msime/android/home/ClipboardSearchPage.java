package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.fragment.app.FragmentActivity;
import androidx.lifecycle.ViewModel;
import androidx.lifecycle.ViewModelProvider;
import app.msime.android.ClipboardHistory;
import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.ClipboardHistoryStore;
import app.msime.android.ClipboardSearchPolicy;
import app.msime.android.HostDeepLink;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import app.msime.android.account.DeviceDataApi;
import java.util.List;

/**
 * 可搜索的本机剪贴板历史（#5973）：顶部是搜索框，下面是筛出来的记录。键盘剪贴板面板顶行点「搜索」打开这一页，设置首页也能搜到它。
 *
 * <p>键盘里没有可输入的文本框，查询框放在应用里，和编辑（#5971）、常用语（#5673）一样。读的是键盘写的同一份共享存储（{@link ClipboardHistoryStore}），所以和面板里看到的是同一份历史；只搜本机历史，云剪贴板有自己的页面。筛选规则在 {@link ClipboardSearchPolicy}：不区分大小写的子串匹配，顺序不变（置顶在前），空查询显示全部。
 *
 * <p>一条记录：点按复制回系统剪贴板；行尾「编辑」打开 {@link ClipboardEditPage}，「删除」从共享存储里删掉这一条。从键盘打开时（深链参数带 {@link HostDeepLink#ARG_EXTERNAL}）复制之后回到原来的应用，接着就能粘贴；从应用里打开时留在这一页。
 */
public final class ClipboardSearchPage extends DetailPage {
    private enum State { LOADING, READY, FAILED }

    private State state = State.LOADING;
    /** 共享存储里的全部条目，顺序就是存储给的顺序；读到之前为空。 */
    private List<ClipboardHistory.Item> items = List.of();
    /** 查询框里的文字，见 {@link Query}；{@link #onCreate} 里取到。 */
    private Query query;
    /** 删除还没回来时不再接受操作，免得同一条被删两次或删完又被编辑。 */
    private boolean busy;
    @Nullable private GroupCard results;

    /** 查询框里的文字放在 ViewModel 里：旋转、换深浅模式会重建 Activity，Fragment 换成新实例，字段会回到空串。 */
    public static final class Query extends ViewModel {
        String text = "";
        /** 第一次显示时把焦点给查询框并拉起键盘；重建后不再拉。 */
        boolean focused;
    }

    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        query = new ViewModelProvider(this).get(Query.class);
    }

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        Context context = column.getContext();
        SearchPill search = new SearchPill(context);
        search.setHint("搜索剪贴板历史");
        search.field().setContentDescription("搜索剪贴板历史");
        search.field().setText(query.text);
        search.field().setSelection(search.field().length());
        search.setOnQueryChange(text -> {
            query.text = text;
            renderResults();
        });
        column.addView(search, Ui.matchWidth());

        results = GroupCard.add(column, null).withDividers(Ui.ROW_PADDING_H);
        // 读取放在 onBecameVisible：每次显示都会调到它，这里再读一次就重复了。
        renderResults();

        if (!query.focused) {
            query.focused = true;
            // 打开这一页就是要搜，不用再点一下搜索框。
            search.field().requestFocus();
            WindowCompat.getInsetsController(requireActivity().getWindow(), search.field())
                .show(WindowInsetsCompat.Type.ime());
        }
    }

    @Override public void onDestroyView() {
        results = null;
        // 视图没了，删除的回调就不会再交回来；不清掉的话这一页之后的操作全被挡住。
        busy = false;
        super.onDestroyView();
    }

    /** 第一次显示、从编辑页回来、应用回到前台时都重读：键盘可能刚记下新复制的内容，编辑页可能刚改了一条。 */
    @Override protected void onBecameVisible() {
        if (getView() != null && !busy) reload();
    }

    private void reload() {
        HostTask.run(this, context -> new ClipboardHistoryStore(context.getFilesDir()).load(), loaded -> {
            if (loaded == null) {
                state = State.FAILED;
            } else {
                items = loaded;
                state = State.READY;
            }
            renderResults();
        });
    }

    /** 只重画结果卡片，搜索框不动：重建它会丢掉焦点和输入法的组字。 */
    private void renderResults() {
        GroupCard card = results;
        if (card == null) return;
        Context context = card.card().getContext();
        card.card().removeAllViews();
        switch (state) {
            case LOADING -> card.note("正在读取…");
            case FAILED -> card.note("剪贴板历史读取失败，请稍后重试");
            case READY -> {
                if (items.isEmpty()) {
                    card.note("剪贴板历史里还没有记录。开启剪贴板历史后，用水杉键盘时复制的文字会记在这里。");
                    return;
                }
                List<ClipboardHistory.Item> shown = ClipboardSearchPolicy.filter(items, query.text);
                if (shown.isEmpty()) {
                    card.note(ClipboardSearchPolicy.emptyMessage(query.text));
                    return;
                }
                long now = System.currentTimeMillis();
                for (ClipboardHistory.Item item : shown) card.addView(itemRow(context, item, now));
            }
        }
    }

    private View itemRow(Context context, ClipboardHistory.Item item, long now) {
        LinearLayout row = Ui.row(context);
        ViewPolicy.setCenteredVertically(row);
        Ui.setPaddingDp(row, context, 16, 12, 8, 12);

        LinearLayout texts = Ui.column(context);
        TextView text = Ui.styledLabel(context, item.text(), 15, 400, Ui.text(context));
        ViewPolicy.setMaxLinesEllipsized(text, 3);
        texts.addView(text);
        String meta = meta(item, now);
        if (!meta.isEmpty()) {
            TextView label = Ui.styledLabel(context, meta, 12, 400, Ui.subText(context));
            LinearLayout.LayoutParams metaParams = Ui.wrap();
            metaParams.topMargin = Ui.dp(context, 4);
            texts.addView(label, metaParams);
        }
        row.addView(texts, Ui.weightWrap(1f));

        row.addView(Ui.iconButton(context, R.drawable.ic_ms_edit, Ui.subText(context), "编辑", 40, () -> edit(item)));
        row.addView(Ui.iconButton(context, R.drawable.ic_ms_delete, Ui.subText(context), "删除", 40, () -> delete(item)));

        row.setContentDescription(item.text() + (meta.isEmpty() ? "" : "，" + meta) + "，点按复制");
        Ui.makeClickable(row, context, () -> copy(item));
        return row;
    }

    /** 「已置顶 · N 分钟前」，没有的部分省掉。 */
    private static String meta(ClipboardHistory.Item item, long now) {
        String when = DeviceDataApi.relativeTime(now, item.timestamp());
        if (!item.pinned()) return when;
        return when.isEmpty() ? "已置顶" : "已置顶 · " + when;
    }

    private void copy(ClipboardHistory.Item item) {
        if (busy) return;
        ClipboardActions.copyText(requireContext(), "水杉剪贴板历史", item.text(), "已复制");
        if (fromKeyboard()) leave();
    }

    /** 交给编辑页的和键盘一样是认出这一条的键，不是文字；编辑页保存后弹回这一页，{@link #onBecameVisible} 重读。 */
    private void edit(ClipboardHistory.Item item) {
        if (busy) return;
        Bundle args = new Bundle();
        args.putString(ClipboardHistoryPolicy.EDIT_ENTRY_ARG,
            ClipboardHistoryPolicy.editKey(item.timestamp(), item.text()));
        SettingsNavigator.open(requireContext(), PageId.CLIPBOARD_EDIT, args);
    }

    /** 和面板里的左滑删除一样不再确认；共享存储按文字删，那一条已经不在时什么也不做。 */
    private void delete(ClipboardHistory.Item item) {
        if (busy) return;
        busy = true;
        String text = item.text();
        HostTask.run(this, context -> {
            new ClipboardHistoryStore(context.getFilesDir()).remove(text);
            return Boolean.TRUE;
        }, done -> {
            busy = false;
            MsToast.show(requireContext(), done == null ? "删除失败，请稍后重试" : "已删除");
            reload();
        });
    }

    /** 深链打开的（键盘的「搜索」）参数里带外部标记；应用里经 {@link SettingsNavigator} 打开的没有。 */
    private boolean fromKeyboard() {
        Bundle args = getArguments();
        return args != null && args.getBoolean(HostDeepLink.ARG_EXTERNAL, false);
    }

    /** 关掉这一页并回到原来的应用，和编辑页的「保存」一样：用户是从键盘过来找一条去粘贴的。 */
    private void leave() {
        FragmentActivity activity = getActivity();
        if (activity == null) return;
        activity.getOnBackPressedDispatcher().onBackPressed();
        activity.moveTaskToBack(true);
    }
}
