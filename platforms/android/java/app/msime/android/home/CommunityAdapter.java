package app.msime.android.home;

import android.graphics.drawable.GradientDrawable;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import androidx.recyclerview.widget.RecyclerView;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.KeyboardSkin;
import app.msime.android.R;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.function.Consumer;

/**
 * The community listing in the design's two shapes: skins as a two-column grid of preview cards,
 * dictionaries and reply sets as the rows of one grouped card.
 *
 * <p>A listing only ever holds one kind, so the grid and the grouped card never interleave; the
 * span lookup below still answers per item so a mixed page would lay out rather than crash.
 */
public final class CommunityAdapter extends RecyclerView.Adapter<CommunityAdapter.Holder> {
    static final int COLUMNS = 2;
    private static final int TYPE_SKIN = 0;
    private static final int TYPE_ROW = 1;
    private static final float GROUP_RADIUS_DP = 20f;

    private final List<CommunityCatalog.Item> items = new ArrayList<>();
    private final Consumer<CommunityCatalog.Item> onOpen;

    /**
     * @param onOpen open the detail sheet, which is where a work is looked at and saved
     */
    public CommunityAdapter(Consumer<CommunityCatalog.Item> onOpen) {
        this.onOpen = onOpen;
    }

    /** Replace the listing, for a new kind or a new search. */
    public void set(List<CommunityCatalog.Item> values) {
        items.clear();
        items.addAll(values);
        notifyDataSetChanged();
    }

    /** Append the next page, keeping what is already on screen where it is. */
    public void append(List<CommunityCatalog.Item> values) {
        if (values.isEmpty()) return;
        int start = items.size();
        items.addAll(values);
        notifyItemRangeInserted(start, values.size());
        // The row that used to close the grouped card is now in the middle of it.
        if (start > 0) notifyItemChanged(start - 1);
    }

    public int size() { return items.size(); }

    /** Grid columns an item takes: one for a skin card, the full width for a row. */
    public int span(int position) {
        return position < items.size() && getItemViewType(position) == TYPE_SKIN ? 1 : COLUMNS;
    }

    @Override public int getItemViewType(int position) {
        return items.get(position).kind() == CommunityRequest.Kind.SKIN ? TYPE_SKIN : TYPE_ROW;
    }

    @NonNull @Override public Holder onCreateViewHolder(@NonNull ViewGroup parent, int type) {
        return new Holder(LayoutInflater.from(parent.getContext()).inflate(
            type == TYPE_SKIN ? R.layout.item_community_skin : R.layout.item_community,
            parent, false));
    }

    @Override public void onBindViewHolder(@NonNull Holder holder, int position) {
        CommunityCatalog.Item item = items.get(position);
        KeyboardSkin skin = preview(item);
        holder.swatch.setSkin(skin);
        holder.badge.setText(glyph(item));
        holder.badge.setVisibility(skin == null ? View.VISIBLE : View.GONE);
        if (holder.description != null) {
            holder.description.setText(item.description().isEmpty()
                ? "作者没有写说明。" : item.description());
        }
        if (holder.divider != null) {
            holder.divider.setVisibility(position == 0 ? View.GONE : View.VISIBLE);
            holder.itemView.setBackground(group(holder.itemView, position == 0,
                position == items.size() - 1));
        }
        holder.name.setText(item.name());
        holder.author.setText(author(item));
        holder.rating.setText(rating(item));
        // 卡片上不放设计稿里的「获取」按钮：缩略图只是几枚色块，详情里才按用户自己的布局画出整块键盘，看过再存才不是盲存。看和存都在详情里，点卡片打开它。
        holder.itemView.setOnClickListener(ignored -> onOpen.accept(item));
        holder.itemView.setContentDescription(item.name() + "，" + author(item) + "，"
            + rating(item) + "，点按查看详情");
    }

    /**
     * The design to draw beside a skin's name.
     *
     * <p>Resolved through the same custom-skin path the keyboard renders a saved design with, so
     * the swatch is the skin rather than an approximation of it. Anything unreadable draws nothing.
     */
    @androidx.annotation.Nullable
    static KeyboardSkin preview(CommunityCatalog.Item item) {
        if (item.kind() != CommunityRequest.Kind.SKIN || item.payload() == null) return null;
        try {
            return KeyboardSkin.custom(item.payload(), false);
        } catch (RuntimeException error) {
            return null;
        }
    }

    /** The glyph in the badge: the first character of the name, as the design's 网/码/医 boxes. */
    private static String glyph(CommunityCatalog.Item item) {
        String name = item.name().trim();
        if (name.isEmpty()) return "?";
        return name.substring(0, name.offsetByCodePoints(0, 1));
    }

    /** The slice of the grouped card behind one row: rounded where the group starts and ends. */
    private static GradientDrawable group(View row, boolean first, boolean last) {
        float radius = GROUP_RADIUS_DP * row.getResources().getDisplayMetrics().density;
        float top = first ? radius : 0f;
        float bottom = last ? radius : 0f;
        GradientDrawable card = new GradientDrawable();
        card.setColor(ContextCompat.getColor(row.getContext(), R.color.surface));
        card.setCornerRadii(new float[] {top, top, top, top, bottom, bottom, bottom, bottom});
        return card;
    }

    private static String author(CommunityCatalog.Item item) {
        String author = item.author().isEmpty() ? "匿名作者" : item.author();
        return item.saves() > 0 ? author + " · " + item.saves() + " 次保存" : author;
    }

    private static String rating(CommunityCatalog.Item item) {
        if (item.ratingCount() <= 0) return "暂无评分";
        return String.format(Locale.ROOT, "★ %.1f · %d 人", item.ratingAverage(),
            item.ratingCount());
    }

    @Override public int getItemCount() { return items.size(); }

    static final class Holder extends RecyclerView.ViewHolder {
        final TextView name;
        // Rows only: the grid card leaves the description to the detail sheet.
        @Nullable final TextView description;
        @Nullable final View divider;
        final TextView badge;
        final TextView author;
        final TextView rating;
        final SkinSwatchView swatch;

        Holder(@NonNull View view) {
            super(view);
            name = view.findViewById(R.id.community_item_name);
            description = view.findViewById(R.id.community_item_description);
            divider = view.findViewById(R.id.community_item_divider);
            badge = view.findViewById(R.id.community_item_badge);
            author = view.findViewById(R.id.community_item_author);
            rating = view.findViewById(R.id.community_item_rating);
            swatch = view.findViewById(R.id.community_item_swatch);
        }
    }
}
