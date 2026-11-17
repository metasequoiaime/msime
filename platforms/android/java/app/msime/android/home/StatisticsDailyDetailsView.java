package app.msime.android.home;

import android.content.Context;
import android.view.Gravity;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.TableLayout;
import android.widget.TableRow;
import android.widget.TextView;
import app.msime.android.DimensionPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TypingStatisticsDailyDetails;
import app.msime.android.ViewPolicy;
import java.util.List;

/** 原生按日明细表，复用共享摘要的记录日、分类与未知值口径。 */
final class StatisticsDailyDetailsView {
    private static final List<String> HEADERS = List.of(
        "日期", "总字数", "汉字", "字母", "数字", "标点", "其他", "活跃时长", "字/分");

    private StatisticsDailyDetailsView() {}

    static void add(LinearLayout parent, List<TypingStatisticsDailyDetails.Row> rows) {
        Context context = parent.getContext();
        GroupCard group = GroupCard.add(parent, "按日明细").help(
            "截至今天最近 30 个记录日。速度只统计汉字与字母；超过 10 秒的上屏间隔视为休息。未测量的时长和速度显示为 —。");
        if (rows == null || rows.isEmpty()) {
            group.note(rows == null ? "按日明细暂时读不到，请刷新统计" : "还没有记录");
            return;
        }
        HorizontalScrollView scroll = new HorizontalScrollView(context);
        scroll.setFillViewport(true);
        TableLayout table = new TableLayout(context);
        table.addView(row(context, HEADERS, true));
        for (TypingStatisticsDailyDetails.Row value : rows) {
            table.addView(row(context, TypingStatisticsDailyDetails.cells(value), false));
        }
        scroll.addView(table, LayoutPolicy.wrapParams());
        group.card().addView(scroll, LayoutPolicy.matchWidthWrapParams());
    }

    private static TableRow row(Context context, List<String> cells, boolean heading) {
        TableRow row = new TableRow(context);
        ViewPolicy.setMinimumHeight(row, DimensionPolicy.pixels(context, 44));
        for (int index = 0; index < cells.size(); index++) {
            TextView cell = ViewPolicy.label(context, cells.get(index), 13,
                heading ? ThemeColorPolicy.subText(context) : ThemeColorPolicy.text(context));
            ViewPolicy.setPaddingDp(context, cell, 12, 12, 12, 12);
            ViewPolicy.setMinimumWidth(cell, DimensionPolicy.pixels(context, index == 0 ? 112 : 88));
            cell.setGravity((index == 0 ? Gravity.START : Gravity.END) | Gravity.CENTER_VERTICAL);
            if (heading) androidx.core.view.ViewCompat.setAccessibilityHeading(cell, true);
            else cell.setContentDescription(HEADERS.get(index) + " " + cells.get(index));
            row.addView(cell);
        }
        return row;
    }
}
