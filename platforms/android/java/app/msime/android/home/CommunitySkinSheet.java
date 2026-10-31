package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.KeyboardSkin;
import app.msime.android.ViewPolicy;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.chip.Chip;
import com.google.android.material.chip.ChipGroup;
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
     * @param layout 预览画读者自己的键盘（{@link KeyboardPreview#layout}）：作品不带作者的键位，读者想看的是这款皮肤装到自己键盘上的样子；没有偏好时传 {@link KeyLayoutStylesPolicy.Preview#STANDARD}
     * @param actionLabel what the sheet's own button says, the same words as the card's pill (获取 / 使用 / 添加 / 已添加); empty for kinds that cannot be taken on this device
     * @param onAction runs when the sheet's own button is pressed; null when there is nothing to do (a reply template, or a pack already added), which gets a disabled button rather than a dead one
     * @param onReport runs when 举报 is pressed, after the sheet closes
     * @param onBlock 点「屏蔽作者」、面板关上之后调用；自己的作品为 null，不显示这个按钮
     * @param onChangeCategory 作者选了另一个分类时调用；不是作者（或不是皮肤）时为 null，详情里只显示分类、不给修改入口
     */
    public static void show(Context context, CommunityCatalog.Item item, KeyLayoutStylesPolicy.Preview layout,
            String actionLabel, Runnable onAction, Runnable onReport, Runnable onBlock,
            Consumer<CommunityRequest.Category> onChangeCategory) {
        KeyboardSkin skin = CommunityAdapter.preview(item);
        SettingsSheet sheet = new SettingsSheet(context, item.name(), subtitle(item));

        if (skin != null) {
            KeyboardPreview preview = new KeyboardPreview(context);
            // 用当前的布局画：用九键的人要看的是九键，底行也是自己选的样式，不是一张跟自己键盘对不上的图。角标说的是画的哪种布局，不是皮肤名——名字就在上面那行标题里。
            preview.setKeyboard(skin, layout, layout.caption());
            LinearLayout.LayoutParams params = LayoutPolicy.matchWidthHeightDp(context, 196);
            params.topMargin = DimensionPolicy.pixels(context, 4);
            // add() fixes every row at WRAP_CONTENT, and this view measures to nothing under it.
            sheet.content().addView(preview, params);
        }

        TextView description = ViewPolicy.styledLabel(context,
            item.description().isEmpty() ? "作者没有写说明。" : item.description(),
            14, 400, ThemeColorPolicy.subText(context));
        LinearLayout.LayoutParams text = LayoutPolicy.matchWidthWrapParams();
        text.topMargin = DimensionPolicy.pixels(context, 14);
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
                ViewPolicy.bindClick(chip, () -> {
                    if (category == item.category()) return;
                    sheet.dismiss();
                    onChangeCategory.accept(category);
                });
            }
            sheet.content().addView(categories, LayoutPolicy.matchWidthWrapParams());
        }

        MaterialButton save = new MaterialButton(context);
        save.setText(actionLabel == null || actionLabel.isEmpty() ? "暂不支持导入" : actionLabel);
        ViewPolicy.setEnabled(save, onAction != null);
        if (onAction != null) {
            ViewPolicy.bindClick(save, () -> {
                sheet.dismiss();
                onAction.run();
            });
        }
        LinearLayout.LayoutParams action = LayoutPolicy.matchWidthWrapParams();
        action.topMargin = DimensionPolicy.pixels(context, 18);
        sheet.content().addView(save, action);

        // Everything here is someone else's work, published without review first; this is how a reader flags it to the moderators.
        // 「屏蔽作者」和「举报」并排靠右：举报交给管理员处理这一件作品，屏蔽让这个账号从此看不到这位作者的全部作品。
        LinearLayout moderation = LayoutPolicy.row(context);
        if (onBlock != null) {
            MaterialButton block = new MaterialButton(context, null,
                androidx.appcompat.R.attr.borderlessButtonStyle);
            block.setText("屏蔽作者");
            ViewPolicy.bindClick(block, () -> {
                sheet.dismiss();
                onBlock.run();
            });
            moderation.addView(block, LayoutPolicy.wrapParams());
        }
        MaterialButton report = new MaterialButton(context, null,
            androidx.appcompat.R.attr.borderlessButtonStyle);
        report.setText("举报");
        ViewPolicy.bindClick(report, () -> {
            sheet.dismiss();
            onReport.run();
        });
        moderation.addView(report, LayoutPolicy.wrapParams());
        LinearLayout.LayoutParams reportParams = LayoutPolicy.wrapParams();
        reportParams.gravity = android.view.Gravity.END;
        reportParams.topMargin = DimensionPolicy.pixels(context, 4);
        sheet.content().addView(moderation, reportParams);

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
        String category = item.category() == null ? null : item.category().label();
        String details;
        if (item.kind() == CommunityRequest.Kind.SKIN) {
            details = CommunityAdapter.author(item) + " · "
                + CommunityRequest.usesLabel(item.downloads());
        } else {
            String meta = CommunityAdapter.subtitle(item);
            details = meta.isEmpty() ? "匿名作者" : meta;
        }
        String rating = CommunityAdapter.rating(item);
        int capacity = (category == null ? 0 : category.length() + 3)
            + details.length() + rating.length() + 3;
        StringBuilder value = new StringBuilder(capacity);
        if (category != null) value.append(category).append(" · ");
        value.append(details).append(" · ").append(rating);
        return value.toString();
    }

    /** Whether this entry can be saved, and so whether the sheet gets a live button. */
    public static boolean installable(CommunityCatalog.Item item) {
        return item.kind() == CommunityRequest.Kind.SKIN && item.payload() != null;
    }
}
