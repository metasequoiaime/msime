package app.msime.android.home;

import android.content.Context;
import android.view.Gravity;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import app.msime.android.DimensionPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TypingKeyLanguages.Language;
import app.msime.android.ViewPolicy;
import java.util.List;
import java.util.function.Consumer;

/** 按键页的原生语言筛选，窗口和统计开关由页面持有。 */
final class StatisticsKeyLanguagePicker {
    private StatisticsKeyLanguagePicker() {}

    static void add(LinearLayout parent, List<Language> available, Language selected,
                    Consumer<Language> onSelect) {
        Context context = parent.getContext();
        LinearLayout row = new LinearLayout(context);
        row.setGravity(Gravity.CENTER_VERTICAL);
        ViewPolicy.setPaddingDp(context, row, 4, 0, 0, 4);
        row.addView(ViewPolicy.label(context, "语言", 14, ThemeColorPolicy.subText(context)),
            new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        TextView picker = ViewPolicy.label(context, selected == null ? "全部" : selected.label(),
            14, ThemeColorPolicy.accent(context));
        ViewPolicy.setMinimumHeightDp(context, picker, 48);
        ViewPolicy.setMinimumWidth(picker, DimensionPolicy.pixels(context, 80));
        ViewPolicy.setPaddingDp(context, picker, 12, 0, 12, 0);
        picker.setGravity(Gravity.CENTER);
        picker.setContentDescription("按键语言：" + picker.getText());
        picker.setFocusable(true);
        picker.setOnClickListener(view -> {
            PopupMenu menu = new PopupMenu(context, picker);
            menu.getMenu().add("全部").setCheckable(true).setChecked(selected == null)
                .setOnMenuItemClickListener(item -> { onSelect.accept(null); return true; });
            for (Language language : available)
                menu.getMenu().add(language.label()).setCheckable(true).setChecked(language == selected)
                    .setOnMenuItemClickListener(item -> { onSelect.accept(language); return true; });
            menu.show();
        });
        row.addView(picker);
        parent.addView(row, LayoutPolicy.matchWidthWrapParams());
    }
}
