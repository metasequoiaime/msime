package app.msime.android.home;

import android.content.Context;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.KeyboardSkin;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.chip.Chip;
import com.google.android.material.chip.ChipGroup;
import java.util.Locale;
import java.util.function.Consumer;

/**
 * 一款社区作品的详情：先看清楚，再决定要不要存。
 *
 * <p>A card can only afford a 52dp swatch, and a swatch is not enough to choose a colour scheme by.
 * Without this, seeing what a skin actually looks like meant saving it, opening the keyboard and
 * picking it in the skin panel — three steps, and a write to the skin library, to answer the
 * question the listing exists to answer.
 *
 * <p>The preview is the same drawing the 键盘 tab shows for the keyboard in use, resolved through
 * the same custom-skin path, so what is on this sheet is what the keyboard will look like.
 */
public final class CommunitySkinSheet {
    private CommunitySkinSheet() {}

    /**
     * Show one entry.
     *
     * @param nineKey draw the preview as the layout this user types on
     * @param actionLabel what the sheet's own button says, the same words as the card's pill (获取 / 使用 / 添加 / 已添加); empty for kinds that cannot be taken on this device
     * @param onAction runs when the sheet's own button is pressed; null when there is nothing to do (a reply template, or a pack already added), which gets a disabled button rather than a dead one
     * @param onReport runs when 举报 is pressed, after the sheet closes
     * @param onChangeCategory 作者选了另一个分类时调用；不是作者（或不是皮肤）时为 null，详情里只显示分类、不给修改入口
     */
    public static void show(Context context, CommunityCatalog.Item item, boolean nineKey,
            String actionLabel, Runnable onAction, Runnable onReport,
            Consumer<CommunityRequest.Category> onChangeCategory) {
        KeyboardSkin skin = CommunityAdapter.preview(item);
        SettingsSheet sheet = new SettingsSheet(context, item.name(), subtitle(item));
        float density = context.getResources().getDisplayMetrics().density;

        if (skin != null) {
            KeyboardPreview preview = new KeyboardPreview(context);
            // 用当前的布局画：用九键的人要看的是九键，不是一张跟自己键盘对不上的图。角标说的是画的
            // 哪种布局，不是皮肤名——名字就在上面那行标题里。
            preview.setKeyboard(skin, nineKey, nineKey ? "九键" : "26 键");
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, Math.round(196 * density));
            params.topMargin = Math.round(4 * density);
            // add() fixes every row at WRAP_CONTENT, and this view measures to nothing under it.
            sheet.content().addView(preview, params);
        }

        TextView description = new TextView(context);
        description.setText(item.description().isEmpty() ? "作者没有写说明。" : item.description());
        description.setTextSize(14);
        description.setTextColor(Ui.subText(context));
        LinearLayout.LayoutParams text = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        text.topMargin = Math.round(14 * density);
        sheet.content().addView(description, text);

        if (onChangeCategory != null && item.category() != null) {
            // 作者自己的皮肤：当前分类先选中，点另一个就交给调用方去改，结果在列表页提示。
            sheet.addHeading("分类");
            ChipGroup categories = new ChipGroup(context);
            categories.setSingleSelection(true);
            categories.setSelectionRequired(true);
            for (CommunityRequest.Category category : CommunityRequest.categories()) {
                Chip chip = new Chip(context);
                chip.setId(android.view.View.generateViewId());
                chip.setText(category.label());
                chip.setCheckable(true);
                categories.addView(chip);
                if (category == item.category()) categories.check(chip.getId());
                chip.setOnClickListener(ignored -> {
                    if (category == item.category()) return;
                    sheet.dismiss();
                    onChangeCategory.accept(category);
                });
            }
            sheet.content().addView(categories, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        }

        MaterialButton save = new MaterialButton(context);
        save.setText(actionLabel == null || actionLabel.isEmpty() ? "暂不支持导入" : actionLabel);
        save.setEnabled(onAction != null);
        if (onAction != null) {
            save.setOnClickListener(ignored -> {
                sheet.dismiss();
                onAction.run();
            });
        }
        LinearLayout.LayoutParams action = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        action.topMargin = Math.round(18 * density);
        sheet.content().addView(save, action);

        // Everything here is someone else's work, published without review first; this is how a reader flags it to the moderators.
        MaterialButton report = new MaterialButton(context, null,
            androidx.appcompat.R.attr.borderlessButtonStyle);
        report.setText("举报");
        report.setOnClickListener(ignored -> {
            sheet.dismiss();
            onReport.run();
        });
        LinearLayout.LayoutParams reportParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        reportParams.gravity = android.view.Gravity.END;
        reportParams.topMargin = Math.round(4 * density);
        sheet.content().addView(report, reportParams);

        sheet.addNote(note(item));
        sheet.show();
    }

    /** What taking this kind of work does, under the button. */
    private static String note(CommunityCatalog.Item item) {
        return switch (item.kind()) {
            case SKIN -> "获取后存进皮肤库，点「使用」才会换上；也可以在键盘的皮肤面板里选用。";
            case DICTIONARY -> "添加后作为一个命名词库启用，可以在设置的词库页停用或删除。";
            case PHRASE -> "添加后出现在常用语里，可以在键盘的常用语面板直接上屏。";
            // 回复模板是 AI 回复用的提示词，手机上还没有本地的模板库可以装进去。
            case REPLY -> "AI 回复模板还不能在手机上添加，这里只能先看看。";
        };
    }

    /** Category, author, uses or entry count, and rating on one line — everything the card shows except the name. */
    private static String subtitle(CommunityCatalog.Item item) {
        StringBuilder value = new StringBuilder();
        if (item.category() != null) value.append(item.category().label()).append(" · ");
        if (item.kind() == CommunityRequest.Kind.SKIN) {
            value.append(CommunityAdapter.author(item)).append(" · ")
                .append(CommunityRequest.usesLabel(item.downloads()));
        } else {
            String meta = CommunityAdapter.subtitle(item);
            value.append(meta.isEmpty() ? "匿名作者" : meta);
        }
        value.append(" · ").append(item.ratingCount() <= 0 ? "暂无评分"
            : String.format(Locale.ROOT, "★ %.1f · %d 人", item.ratingAverage(),
                item.ratingCount()));
        return value.toString();
    }

    /** Whether this entry can be saved, and so whether the sheet gets a live button. */
    public static boolean installable(CommunityCatalog.Item item) {
        return item.kind() == CommunityRequest.Kind.SKIN && item.payload() != null;
    }
}
