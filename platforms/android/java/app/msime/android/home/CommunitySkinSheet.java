package app.msime.android.home;

import android.content.Context;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.core.content.ContextCompat;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.KeyboardSkin;
import app.msime.android.R;
import com.google.android.material.button.MaterialButton;
import java.util.Locale;

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
     * @param onSave  runs when the sheet's own button is pressed; null for kinds that cannot be
     *                imported yet, which get a disabled button rather than a dead one
     */
    public static void show(Context context, CommunityCatalog.Item item, boolean nineKey,
            Runnable onSave) {
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
        description.setTextColor(ContextCompat.getColor(context, R.color.text_secondary));
        LinearLayout.LayoutParams text = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        text.topMargin = Math.round(14 * density);
        sheet.content().addView(description, text);

        MaterialButton save = new MaterialButton(context);
        save.setText(onSave == null ? "暂不支持导入" : "保存到皮肤库");
        save.setEnabled(onSave != null);
        if (onSave != null) {
            save.setOnClickListener(ignored -> {
                sheet.dismiss();
                onSave.run();
            });
        }
        LinearLayout.LayoutParams action = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        action.topMargin = Math.round(18 * density);
        sheet.content().addView(save, action);

        sheet.addNote(onSave == null
            // 词库和回复要先有本地编辑器才谈得上导入，那一页还没搬过来。
            ? "词库和回复还不能在手机上导入，这里只能先看看。"
            : "保存后在键盘的皮肤面板里选用，当前皮肤不会被换掉。");
        sheet.show();
    }

    /** Author, saves and rating on one line — everything the card shows except the name. */
    private static String subtitle(CommunityCatalog.Item item) {
        StringBuilder value = new StringBuilder(
            item.author().isEmpty() ? "匿名作者" : item.author());
        if (item.saves() > 0) value.append(" · ").append(item.saves()).append(" 次保存");
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
