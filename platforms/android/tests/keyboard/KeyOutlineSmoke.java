package app.msime.android;

/**
 * 键帽轮廓：每个像素行能落墨的范围与键形的解析解一致（圆角和胶囊按圆，比键高窄的胶囊按 Skia 的规则缩成竖着的跑道形，票根的缺口，卵石的右上角），扣掉边距、内缩和凸起厚度；右上角的框放不下时沿对角线挪进来，再挪也放不下时给 NaN。
 */
public final class KeyOutlineSmoke {
    private KeyOutlineSmoke() { }

    /** 圆弧按每四分之一圈 16 段展平；切线接近水平的顶行误差最大，半径 50 像素时约 0.3 像素。 */
    private static final float TOLERANCE = 0.5f;

    public static void main(String[] args) {
        // 没有皮肤形状：整个键格都能落墨，右上角不用挪。
        KeyOutline.Spans none = KeyOutline.NONE.spans(100, 50);
        check(near(none.left(0), 0) && near(none.right(49), 100), "无轮廓时整个键格都能落墨");
        check(none.cornerOffset(100, 0, 10, 10) == 0, "无轮廓时右上角不用挪");

        // 内缩：方角键帽只剩去掉四边内缩的那块。
        KeyOutline.Spans inset = new KeyOutline(KeyOutline.Shape.ROUNDED, 0, 3, 0, 0).spans(100, 50);
        check(empty(inset, 2) && near(inset.left(3), 3) && near(inset.right(3), 97) && empty(inset, 47),
            "内缩 3 像素：第 3–46 行从 3 到 97");

        // 边距：离轮廓至少 2 像素，上下两行和左右两列都不落墨。
        KeyOutline.Spans margin = new KeyOutline(KeyOutline.Shape.ROUNDED, 0, 0, 0, 2).spans(100, 50);
        check(empty(margin, 0) && empty(margin, 1) && near(margin.left(2), 2) && near(margin.right(25), 98)
            && empty(margin, 48), "边距 2 像素：四边各让出 2 像素");

        // 宽胶囊：两端是半径为键高一半的半圆。
        KeyOutline.Spans capsule = new KeyOutline(KeyOutline.Shape.CAPSULE, 0, 0, 0, 0).spans(200, 100);
        check(within(capsule.left(1), 50 - Math.sqrt(50 * 50 - 48.5 * 48.5)), "宽胶囊顶上一行从圆弧开始");
        check(within(capsule.left(50), 0) && within(capsule.right(50), 200), "宽胶囊中线横贯整个键格");

        // 比键高窄的胶囊：Skia 把半径缩到键宽的一半，成了上下两头圆的跑道形，中段是直边。
        KeyOutline.Spans narrow = new KeyOutline(KeyOutline.Shape.CAPSULE, 0, 0, 0, 0).spans(60, 100);
        check(within(narrow.left(1), 30 - Math.sqrt(30 * 30 - 28.5 * 28.5)), "窄胶囊顶上按半径 30 的圆");
        check(within(narrow.left(40), 0) && within(narrow.right(40), 60), "窄胶囊中段是直边（半径没有超出键宽）");

        // 圆角：半径不超过短边的一半。
        KeyOutline.Spans rounded = new KeyOutline(KeyOutline.Shape.ROUNDED, 80, 0, 0, 0).spans(100, 40);
        check(within(rounded.left(20), 0) && within(rounded.left(0), 20 - Math.sqrt(20 * 20 - 19.5 * 19.5)),
            "圆角半径收到短边的一半");

        // 票根：左右两侧中间各有一个半径为短边 12% 的半圆缺口。
        KeyOutline.Spans ticket = new KeyOutline(KeyOutline.Shape.TICKET, 0, 0, 0, 0).spans(100, 60);
        float notch = 60 * .12f;
        check(within(ticket.right(29), 100 - Math.sqrt(notch * notch - .25)), "票根缺口在中线把右边界收进来");
        check(within(ticket.left(29), Math.sqrt(notch * notch - .25)), "票根缺口在中线把左边界收进来");
        check(within(ticket.right(5), 100), "票根缺口以外是直边");
        check(ticket.sideInset(26, 34) > notch - 1, "票根缺口那几行两侧都要让出缺口的深度");

        // 凸起：键帽面下面的厚度不落墨。
        KeyOutline.Spans raised = new KeyOutline(KeyOutline.Shape.ROUNDED, 0, 0, 9, 0).spans(100, 50);
        check(!empty(raised, 40) && empty(raised, 41), "凸起 9 像素：最下面 9 行不落墨");

        // 卵石：右上角收得最多，贴着键格右上角的框要往里挪，挪进来以后整个在键帽里。
        KeyOutline pebble = new KeyOutline(KeyOutline.Shape.PEBBLE, 0, 0, 0, 2);
        KeyOutline.Spans stone = pebble.spans(100, 140);
        check(!stone.fits(85, 4, 95, 16), "卵石右上角贴边的框在键帽外");
        float offset = stone.cornerOffset(95, 4, 10, 12);
        check(offset > 0 && stone.fits(85 - offset, 4 + offset, 95 - offset, 16 + offset), "卵石右上角的框沿对角线挪进键帽");
        check(stone.cornerOffset(95, 4, 10, 12) == offset && pebble.spans(100, 140) == stone, "同尺寸的表来自缓存");
        // 卵石按键宽缩放：宽的键右上角收进去的像素更多。
        check(pebble.spans(150, 140).cornerOffset(145, 4, 10, 12) > offset, "宽的卵石键右上角挪得更多");

        // 胶囊的圆形小键：太大的框怎么挪都放不下。
        KeyOutline.Spans circle = new KeyOutline(KeyOutline.Shape.CAPSULE, 0, 0, 0, 2).spans(60, 60);
        check(Float.isNaN(circle.cornerOffset(60, 0, 50, 50)), "圆形小键里放不下的框给 NaN");
        check(circle.cornerOffset(55, 3, 8, 10) > 0, "圆形小键的角标往里挪");

        // 键形名。
        check(KeyOutline.Shape.of("capsule") == KeyOutline.Shape.CAPSULE && KeyOutline.Shape.of("ticket") == KeyOutline.Shape.TICKET
            && KeyOutline.Shape.of("pebble") == KeyOutline.Shape.PEBBLE && KeyOutline.Shape.of("rounded") == KeyOutline.Shape.ROUNDED
            && KeyOutline.Shape.of(null) == KeyOutline.Shape.ROUNDED && KeyOutline.Shape.of("unknown") == KeyOutline.Shape.ROUNDED,
            "键形名按偏好的取值");
        check(new KeyOutline(KeyOutline.Shape.PEBBLE, 0, 1, 0, 2).equals(new KeyOutline(KeyOutline.Shape.PEBBLE, 0, 1, 0, 2)),
            "同样的轮廓相等（键组按它判断要不要重算）");
        System.out.println("KeyOutlineSmoke passed");
    }

    private static boolean empty(KeyOutline.Spans spans, int row) { return spans.left(row) > spans.right(row); }

    private static boolean within(float actual, double expected) { return Math.abs(actual - expected) <= TOLERANCE; }

    private static boolean near(float actual, float expected) { return Math.abs(actual - expected) < 0.01f; }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
