package app.msime.android.home;

import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.EditorInfo;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import androidx.recyclerview.widget.GridLayoutManager;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.R;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;
import com.google.android.material.snackbar.Snackbar;
import com.google.android.material.textfield.TextInputEditText;
import java.nio.file.Paths;

/**
 * The 社区 tab: skins, dictionaries and reply templates published by other people.
 *
 * Read-only. Publishing and rating need a signed-in account, and this host carries only the
 * keyboard's anonymous identity; a publish button that always answers "请先登录" would be worse
 * than the honest absence of one. Saving a skin, which is what people open this tab to do, works.
 */
public final class CommunityFragment extends Fragment {
    private static final String ARG_KIND = "kind";

    private CommunityRequest.Kind kind = CommunityRequest.Kind.SKIN;
    private CommunityAdapter adapter;
    private String search = "";
    private boolean loading;
    private boolean hasMore;
    // 详情里的预览按用户自己的布局画。读不到就按 26 键，那是默认值。
    private boolean nineKey;

    /** The tab, opened on one kind of work; a null kind opens on skins. */
    public static CommunityFragment forKind(@Nullable CommunityRequest.Kind kind) {
        CommunityFragment fragment = new CommunityFragment();
        if (kind != null) {
            Bundle arguments = new Bundle();
            arguments.putString(ARG_KIND, kind.id());
            fragment.setArguments(arguments);
        }
        return fragment;
    }

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_community, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        Bundle arguments = getArguments();
        if (arguments != null) {
            for (CommunityRequest.Kind value : CommunityRequest.kinds()) {
                if (value.id().equals(arguments.getString(ARG_KIND))) kind = value;
            }
        }
        // The design's segmented control. The opening kind is checked before the listener is attached, so opening loads the listing once, below, rather than once per path.
        MaterialButtonToggleGroup kinds = view.findViewById(R.id.community_kinds);
        LayoutInflater inflater = LayoutInflater.from(requireContext());
        java.util.List<CommunityRequest.Kind> values = CommunityRequest.kinds();
        int[] segments = new int[values.size()];
        for (int index = 0; index < values.size(); index++) {
            MaterialButton segment =
                (MaterialButton) inflater.inflate(R.layout.item_segment, kinds, false);
            segment.setId(View.generateViewId());
            segment.setText(values.get(index).title());
            kinds.addView(segment);
            segments[index] = segment.getId();
        }
        kinds.check(segments[Math.max(0, values.indexOf(kind))]);
        kinds.addOnButtonCheckedListener((group, id, checked) -> {
            if (!checked) return;
            for (int index = 0; index < segments.length; index++) {
                if (segments[index] != id || values.get(index) == kind) continue;
                kind = values.get(index);
                updateSearchHint();
                load(true);
            }
        });

        adapter = new CommunityAdapter(this::open);
        HostTask.run(this, HostStore::loadPreferences, snapshot -> {
            // loadPreferences hands back the whole snapshot; the layout lives one level down.
            org.json.JSONObject preferences =
                snapshot == null ? null : snapshot.optJSONObject("preferences");
            nineKey = preferences != null && "nine_key".equals(
                preferences.optString("touch_keyboard_layout", "twenty_six_key"));
        });
        RecyclerView items = view.findViewById(R.id.community_items);
        GridLayoutManager grid = new GridLayoutManager(requireContext(), CommunityAdapter.COLUMNS);
        grid.setSpanSizeLookup(new GridLayoutManager.SpanSizeLookup() {
            @Override public int getSpanSize(int position) { return adapter.span(position); }
        });
        items.setLayoutManager(grid);
        items.setAdapter(adapter);
        items.addOnScrollListener(new RecyclerView.OnScrollListener() {
            @Override public void onScrolled(@NonNull RecyclerView list, int dx, int dy) {
                if (dy <= 0 || loading || !hasMore) return;
                LinearLayoutManager manager = (LinearLayoutManager) list.getLayoutManager();
                if (manager == null) return;
                // One screen of slack so the next page is already arriving when the last card is
                // reached, rather than after a stop at the bottom.
                if (manager.findLastVisibleItemPosition()
                    >= adapter.size() - CommunityRequest.PAGE_SIZE / 4) load(false);
            }
        });

        TextInputEditText field = view.findViewById(R.id.community_search);
        field.setOnEditorActionListener((text, action, event) -> {
            if (action != EditorInfo.IME_ACTION_SEARCH) return false;
            search = text.getText() == null ? "" : text.getText().toString();
            // The results are what the search was for, and the keyboard is sitting on top of them.
            android.view.inputmethod.InputMethodManager manager =
                requireContext().getSystemService(android.view.inputmethod.InputMethodManager.class);
            if (manager != null) manager.hideSoftInputFromWindow(text.getWindowToken(), 0);
            text.clearFocus();
            load(true);
            return true;
        });

        MaterialButton retry = view.findViewById(R.id.community_retry);
        retry.setOnClickListener(ignored -> load(true));

        load(true);
        updateSearchHint();
    }

    private void updateSearchHint() {
        View view = getView();
        if (view == null) return;
        // The pill field has no floating label, so the hint lives on the text itself.
        ((TextInputEditText) view.findViewById(R.id.community_search)).setHint(kind.searchHint());
    }

    private void load(boolean fresh) {
        View view = getView();
        if (view == null) return;
        // Only paging defers to a request already in flight. A new tab or a new search must go out
        // even mid-load, or switching tabs while the first page is arriving does nothing at all;
        // the reply that was already on its way is discarded below by the same check.
        if (loading && !fresh) return;
        loading = true;
        int offset = fresh ? 0 : adapter.size();
        if (fresh) {
            hasMore = false;
            adapter.set(java.util.List.of());
            state("正在载入…", false);
        }
        CommunityRequest.Kind requested = kind;
        String term = search;
        HostTask.run(this,
            context -> new CommunityCatalog(context).list(requested, term, offset),
            page -> {
                // The tab or the search may have moved on while this page was in flight. A stale
                // answer must not write over what the user is now looking at, and must not clear
                // the flag belonging to the request that replaced it.
                if (requested != kind || !term.equals(search)) return;
                loading = false;
                if (page == null) {
                    state(CommunityRequest.message(null, 0), true);
                    return;
                }
                if (page.failed()) {
                    state(page.failure(), true);
                    return;
                }
                hasMore = page.hasMore();
                if (offset == 0) adapter.set(page.items()); else adapter.append(page.items());
                state(adapter.size() == 0 ? emptyMessage() : "", false);
            });
    }

    private String emptyMessage() {
        return search.isEmpty()
            ? "社区里还没有公开的" + kind.title() + "。"
            : "没有找到匹配「" + search + "」的" + kind.title() + "。";
    }

    private void state(String message, boolean retryable) {
        View view = getView();
        if (view == null) return;
        TextView state = view.findViewById(R.id.community_state);
        state.setText(message);
        state.setVisibility(message.isEmpty() ? View.GONE : View.VISIBLE);
        view.findViewById(R.id.community_retry)
            .setVisibility(retryable ? View.VISIBLE : View.GONE);
    }

    private void open(CommunityCatalog.Item item) {
        CommunitySkinSheet.show(requireContext(), item, nineKey,
            CommunitySkinSheet.installable(item) ? () -> install(item) : null);
    }

    private void install(CommunityCatalog.Item item) {
        HostTask.run(this, context -> {
            String directory = HostStore.directory(context);
            if (directory.isEmpty()) return "键盘还没有完成首次准备，请先打开一次键盘。";
            return new CommunityCatalog(context).install(Paths.get(directory), item);
        }, failure -> {
            View view = getView();
            if (view == null) return;
            Snackbar.make(view, failure == null || failure.isEmpty()
                ? "已保存到皮肤库，在键盘的皮肤面板里选用。" : failure,
                Snackbar.LENGTH_LONG).show();
        });
    }
}
