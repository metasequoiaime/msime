package app.msime.android.home;

import android.content.Context;
import android.graphics.drawable.GradientDrawable;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.recyclerview.widget.RecyclerView;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.BoundsPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.KeyboardSkin;
import app.msime.android.R;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.function.Consumer;

/**
 * The community listing in the design's two shapes: skins as a two-column grid of preview cards with a tonal 获取 / 使用 pill, dictionaries, phrase packs and reply templates as the rows of grouped cards with a glyph badge and an 添加 pill.
 *
 * <p>The phrase segment shows two sections, the packs and then 「AI 回复模板」, so the listing can hold a section title between two runs of rows; a run of rows is one grouped card, rounded where the run starts and ends.
 */
public final class CommunityAdapter extends RecyclerView.Adapter<CommunityAdapter.Holder> {
    static final int COLUMNS = 2;
    private static final int TYPE_SKIN = 0;
    private static final int TYPE_ROW = 1;
    private static final int TYPE_HEADER = 2;
    private static final float GROUP_RADIUS_DP = 20f;

    /** What the pill on an entry says: not yet taken, in progress, or taken (a skin then says 使用, a pack 已添加). */
    public enum Action { AVAILABLE, BUSY, DONE }

    /** One line of the listing: an entry, or a section title when `item` is null. */
    private record Entry(@Nullable CommunityCatalog.Item item, String header) {}

    private final ArrayList<Entry> entries = new ArrayList<>(CommunityRequest.PAGE_SIZE);
    private final Map<String, Action> actions = new HashMap<>();
    private final Consumer<CommunityCatalog.Item> onOpen;
    private final Consumer<CommunityCatalog.Item> onAction;
    private boolean nineKey;

    /**
     * @param onOpen open the detail sheet (larger preview, description, 举报)
     * @param onAction the pill was pressed: get or apply a skin, add a dictionary or phrase pack
     */
    public CommunityAdapter(Consumer<CommunityCatalog.Item> onOpen,
            Consumer<CommunityCatalog.Item> onAction) {
        this.onOpen = onOpen;
        this.onAction = onAction;
    }

    /** Ids compare case-insensitively, as the catalogue does (UUIDs may arrive in either case). */
    static String key(String id) {
        return TextPolicy.lowercase(id);
    }

    /** Replace the listing, for a new kind or a new search. */
    public void set(List<CommunityCatalog.Item> values) {
        entries.clear();
        entries.ensureCapacity(values.size());
        for (CommunityCatalog.Item item : values) entries.add(new Entry(item, ""));
        notifyDataSetChanged();
    }

    /** Append the next page, keeping what is already on screen where it is. */
    public void append(List<CommunityCatalog.Item> values) {
        if (values.isEmpty()) return;
        int start = entries.size();
        entries.ensureCapacity(start + values.size());
        for (CommunityCatalog.Item item : values) entries.add(new Entry(item, ""));
        notifyItemRangeInserted(start, values.size());
        // The row that used to close the grouped card is now in the middle of it.
        if (start > 0) notifyItemChanged(start - 1);
    }

    /** Start a new section, such as 「AI 回复模板」 under the phrase packs. */
    public void appendHeader(String title) {
        entries.ensureCapacity(entries.size() + 1);
        entries.add(new Entry(null, title));
        notifyItemInserted(entries.size() - 1);
        if (entries.size() > 1) notifyItemChanged(entries.size() - 2);
    }

    /** Entries plus section titles: the position paging measures against. */
    public int size() { return entries.size(); }

    /** Entries of one kind, so paging a section knows its own offset. */
    public int count(CommunityRequest.Kind kind) {
        int count = 0;
        for (Entry entry : entries) if (entry.item() != null && entry.item().kind() == kind) count++;
        return count;
    }

    /** Whether the listing holds any entry at all (section titles do not count). */
    public boolean hasItems() {
        for (Entry entry : entries) if (entry.item() != null) return true;
        return false;
    }

    /** Draw skin previews as the layout this user types on. */
    public void setNineKey(boolean value) {
        if (nineKey == value) return;
        nineKey = value;
        notifyDataSetChanged();
    }

    /** Set the pill state of every entry named in `values` (keys from {@link #key}); entries not named keep theirs. */
    public void setActions(Map<String, Action> values) {
        actions.putAll(values);
        notifyDataSetChanged();
    }

    /** The pill state of one entry. */
    public void setAction(String id, Action action) {
        String key = key(id);
        actions.put(key, action);
        for (int index = 0; index < entries.size(); index++) {
            CommunityCatalog.Item item = entries.get(index).item();
            if (item != null && key(item.id()).equals(key)) notifyItemChanged(index);
        }
    }

    public Action action(CommunityCatalog.Item item) {
        Action action = actions.get(key(item.id()));
        return action == null ? Action.AVAILABLE : action;
    }

    /**
     * 换上服务端回来的新版本条目（作者改了分类之后）。
     *
     * @param keep 这一款是否还属于当前列表；按分类筛选时改到别的分类就从列表里拿掉
     */
    public void replace(CommunityCatalog.Item updated, boolean keep) {
        for (int index = 0; index < entries.size(); index++) {
            CommunityCatalog.Item item = entries.get(index).item();
            if (item == null || !item.id().equals(updated.id())) continue;
            if (keep) {
                entries.set(index, new Entry(updated, ""));
                notifyItemChanged(index);
            } else {
                entries.remove(index);
                notifyItemRemoved(index);
                // 分组卡片的首尾圆角跟着位置走，移走一行要让相邻的行重画。
                int start = BoundsPolicy.nonNegative(index - 1);
                int end = BoundsPolicy.bounded(index + 1, 0, entries.size());
                if (end > start) notifyItemRangeChanged(start, end - start);
            }
            return;
        }
    }

    /** Grid columns an entry takes: one for a skin card, the full width for a row or a title. */
    public int span(int position) {
        return position < entries.size() && getItemViewType(position) == TYPE_SKIN ? 1 : COLUMNS;
    }

    @Override public int getItemViewType(int position) {
        CommunityCatalog.Item item = entries.get(position).item();
        if (item == null) return TYPE_HEADER;
        return item.kind() == CommunityRequest.Kind.SKIN ? TYPE_SKIN : TYPE_ROW;
    }

    @NonNull @Override public Holder onCreateViewHolder(@NonNull ViewGroup parent, int type) {
        int layout = switch (type) {
            case TYPE_SKIN -> R.layout.item_community_skin;
            case TYPE_HEADER -> R.layout.ms_w5_comm_header;
            default -> R.layout.item_community;
        };
        return new Holder(LayoutInflater.from(parent.getContext()).inflate(layout, parent, false));
    }

    @Override public void onBindViewHolder(@NonNull Holder holder, int position) {
        Entry entry = entries.get(position);
        CommunityCatalog.Item item = entry.item();
        if (item == null) {
            holder.name.setText(entry.header());
            return;
        }
        holder.name.setText(item.name());
        if (holder.badge != null) holder.badge.setText(glyph(item));
        Action action = action(item);
        if (item.kind() == CommunityRequest.Kind.SKIN) bindSkin(holder, item);
        else bindRow(holder, item, position);
        ViewPolicy.bindClick(holder.itemView, () -> onOpen.accept(item));
        bindPill(holder.itemView.getContext(), holder.action, item, action);
    }

    private void bindSkin(Holder holder, CommunityCatalog.Item item) {
        KeyboardSkin skin = preview(item);
        if (holder.preview != null) {
            holder.preview.setCornerRadiusDp(5f);
            holder.preview.setKeyboard(skin, nineKey);
            holder.preview.setVisibility(skin == null ? View.GONE : View.VISIBLE);
        }
        if (holder.badge != null) holder.badge.setVisibility(skin == null ? View.VISIBLE : View.GONE);
        if (holder.category != null) {
            holder.category.setText(item.category() == null ? "" : item.category().label());
            holder.category.setVisibility(item.category() == null ? View.GONE : View.VISIBLE);
        }
        String author = author(item);
        String uses = CommunityRequest.usesLabel(item.downloads());
        if (holder.author != null) holder.author.setText(author);
        if (holder.meta != null) holder.meta.setText(uses);
        holder.itemView.setContentDescription(item.name() + "，"
            + (item.category() == null ? "" : item.category().label() + "分类，")
            + author + "，" + uses + "，" + rating(item) + "，点按查看详情");
    }

    private void bindRow(Holder holder, CommunityCatalog.Item item, int position) {
        boolean first = position == 0 || entries.get(position - 1).item() == null;
        boolean last = position == entries.size() - 1 || entries.get(position + 1).item() == null;
        if (holder.divider != null) holder.divider.setVisibility(first ? View.GONE : View.VISIBLE);
        holder.itemView.setBackground(group(holder.itemView, first, last));
        String subtitle = subtitle(item);
        if (holder.author != null) {
            holder.author.setText(subtitle);
            Ui.setVisibilityForText(holder.author, subtitle);
        }
        boolean reply = item.kind() == CommunityRequest.Kind.REPLY;
        if (holder.description != null) {
            holder.description.setText(item.description());
            holder.description.setVisibility(reply && !item.description().isEmpty()
                ? View.VISIBLE : View.GONE);
        }
        holder.itemView.setContentDescription(item.name() + "，"
            + (subtitle.isEmpty() ? "" : subtitle + "，") + "点按查看详情");
    }

    /** 「@作者 · 4,812 条 · 本周更新」：条数按词库的词条或短语包的短语数，更新时间只在条目带 `updated_at` 时才写。 */
    static String subtitle(CommunityCatalog.Item item) {
        int count = CommunityRequest.entryCount(item.kind(), item.payload());
        Object updated = item.raw() == null ? null : item.raw().opt("updated_at");
        return CommunityRequest.resourceSubtitle(item.author(), count,
            updated instanceof String text
                && CommunityRequest.updatedThisWeek(text, System.currentTimeMillis()));
    }

    /** The tonal pill: 获取 / 使用 on a skin, 添加 / 已添加 on a dictionary or phrase pack, none on a reply template. */
    private void bindPill(Context context, @Nullable TextView pill, CommunityCatalog.Item item,
            Action action) {
        if (pill == null) return;
        String label = label(item, action);
        if (label.isEmpty()) {
            ViewPolicy.hide(pill);
            pill.setOnClickListener(null);
            return;
        }
        ViewPolicy.show(pill);
        pill.setText(label);
        boolean skin = item.kind() == CommunityRequest.Kind.SKIN;
        // 「已添加」是终态：没有底色、正文色、不响应；皮肤拿到之后的「使用」仍是可点的 tonal 按钮。
        boolean enabled = action == Action.AVAILABLE || (skin && action == Action.DONE);
        boolean filled = action != Action.DONE || skin;
        pill.setBackground(filled ? Ui.pillRipple(context, Ui.accentSoft(context)) : null);
        pill.setTextColor(filled ? Ui.accent(context) : Ui.text(context));
        pill.setEnabled(enabled);
        ViewPolicy.setInteractive(pill, enabled);
        ViewPolicy.setActiveAlpha(pill, action != Action.BUSY, 0.6f);
        if (enabled) ViewPolicy.bindClick(pill, () -> onAction.accept(item));
        else pill.setOnClickListener(null);
        pill.setAccessibilityDelegate(KeyboardSheets.buttonDelegate(label + "，" + item.name()));
    }

    /** What the pill says for this entry in this state; empty for an entry without one. */
    static String label(CommunityCatalog.Item item, Action action) {
        return switch (item.kind()) {
            case SKIN -> CommunitySkinSheet.installable(item)
                ? (action == Action.DONE ? "使用" : action == Action.BUSY ? "获取中" : "获取") : "";
            case DICTIONARY, PHRASE -> item.raw() == null ? ""
                : action == Action.DONE ? "已添加" : action == Action.BUSY ? "添加中" : "添加";
            case REPLY -> "";
        };
    }

    /**
     * The design to draw for a skin.
     *
     * <p>Resolved through the same custom-skin path the keyboard renders a saved design with, so the preview is the skin rather than an approximation of it. Anything unreadable draws nothing.
     */
    @Nullable static KeyboardSkin preview(CommunityCatalog.Item item) {
        if (item.kind() != CommunityRequest.Kind.SKIN || item.payload() == null) return null;
        try {
            return KeyboardSkin.custom(item.payload(), false);
        } catch (RuntimeException error) {
            return null;
        }
    }

    /** The glyph in the badge: the first character of the name, as the design's 网/码/医 boxes. */
    static String glyph(CommunityCatalog.Item item) {
        return Ui.trimmedInitial(item.name(), "?");
    }

    /** The slice of the grouped card behind one row: rounded where the group starts and ends. */
    private static GradientDrawable group(View row, boolean first, boolean last) {
        float radius = Ui.dpFloat(row.getContext(), GROUP_RADIUS_DP);
        float top = first ? radius : 0f;
        float bottom = last ? radius : 0f;
        return DrawablePolicy.rounded(Ui.card(row.getContext()),
            new float[] {top, top, top, top, bottom, bottom, bottom, bottom});
    }

    /** 皮肤卡上的作者行：设计写「@作者」，没写作者时是「匿名作者」。 */
    static String author(CommunityCatalog.Item item) {
        return item.author().isEmpty() ? "匿名作者" : "@" + item.author();
    }

    static String rating(CommunityCatalog.Item item) {
        if (item.ratingCount() <= 0) return "暂无评分";
        return String.format(Locale.ROOT, "★ %.1f · %d 人", item.ratingAverage(),
            item.ratingCount());
    }

    @Override public int getItemCount() { return entries.size(); }

    static final class Holder extends RecyclerView.ViewHolder {
        final TextView name;
        @Nullable final TextView badge;
        @Nullable final TextView author;
        // Skin cards: the 使用次数 line. Rows have none.
        @Nullable final TextView meta;
        // Rows only: a reply template's description.
        @Nullable final TextView description;
        @Nullable final View divider;
        // Skin cards only.
        @Nullable final TextView category;
        @Nullable final KeyboardPreview preview;
        @Nullable final TextView action;

        Holder(@NonNull View view) {
            super(view);
            name = view.findViewById(R.id.community_item_name);
            badge = view.findViewById(R.id.community_item_badge);
            author = view.findViewById(R.id.community_item_author);
            meta = view.findViewById(R.id.community_item_rating);
            description = view.findViewById(R.id.community_item_description);
            divider = view.findViewById(R.id.community_item_divider);
            category = view.findViewById(R.id.community_item_category);
            preview = view.findViewById(R.id.community_item_preview);
            action = view.findViewById(R.id.community_item_action);
        }
    }
}
