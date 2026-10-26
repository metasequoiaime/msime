package app.msime.android;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** 按笔画分组的偏旁部首数据；数据来源为 issue #9786 附件 GF 0011-2009 整理表。 */
public final class RadicalPanelModel {
    public static final List<String> STROKES = List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "17");
    private static final Map<String, List<String>> GROUPS = build();
    private RadicalPanelModel() {}

    public static List<String> symbols(String stroke) {
        List<String> values = GROUPS.get(stroke);
        return values == null ? List.of() : values;
    }

    public static int total() {
        int count = 0;
        for (List<String> values : GROUPS.values()) count += values.size();
        return count;
    }

    private static Map<String, List<String>> build() {
        Map<String, List<String>> values = new LinkedHashMap<>();
        values.put("1", List.of(
            "一", "丨", "丿", "丶", "乙", "乛", "乚", "亅", "㇀"
        ));
        values.put("2", List.of(
            "二", "十", "厂", "匚", "卜", "冂", "八", "人", "勹", "儿", "匕", "几",
            "亠", "冫", "冖", "凵", "卩", "刀", "力", "又", "厶", "廴", "亻", "入",
            "⺈", "刂", "㔾", "⺊", "⺁", "⺆", "⺇", "⺂"
        ));
        values.put("3", List.of(
            "干", "工", "土", "艹", "寸", "廾", "大", "尢", "弋", "小", "口", "囗",
            "山", "巾", "彳", "彡", "夕", "夂", "丬", "广", "门", "宀", "辶", "彐",
            "尸", "己", "弓", "子", "屮", "女", "飞", "马", "幺", "巛", "氵", "忄",
            "扌", "犭", "纟", "饣", "讠", "钅", "衤", "礻", "辵", "阝", "阝", "士",
            "夂", "彑", "⺌", "⺍", "⺕", "⺘", "⺙", "⺛", "⺎"
        ));
        values.put("4", List.of(
            "王", "无", "韦", "木", "支", "犬", "歹", "车", "牙", "戈", "比", "瓦",
            "止", "攴", "日", "贝", "水", "见", "牛", "手", "气", "毛", "长", "片",
            "斤", "爪", "父", "月", "氏", "欠", "风", "殳", "文", "方", "火", "斗",
            "户", "心", "毋", "灬", "牜", "爫", "王", "车", "贝", "见", "风", "长",
            "韦", "旡", "鬥", "⺝", "⺡", "⺣"
        ));
        values.put("5", List.of(
            "示", "甘", "石", "龙", "业", "目", "田", "罒", "皿", "生", "矢", "禾",
            "白", "瓜", "鸟", "疒", "立", "穴", "疋", "皮", "癶", "罒", "鸟", "龙",
            "业", "歺", "⺫", "⺬", "⺭", "⺰", "⺱"
        ));
        values.put("6", List.of(
            "耒", "老", "耳", "臣", "覀", "而", "页", "至", "虍", "虫", "肉", "缶",
            "舌", "竹", "臼", "自", "血", "舟", "色", "齐", "衣", "羊", "米", "聿",
            "艮", "羽", "糸", "⺮", "⺷", "糹", "网", "月", "西", "頁", "齊", "爿",
            "⺶"
        ));
        values.put("7", List.of(
            "麦", "走", "赤", "豆", "酉", "辰", "豕", "卤", "里", "足", "邑", "身",
            "釆", "谷", "豸", "龟", "角", "言", "辛", "訁", "⻊", "麥", "鹵", "龜",
            "⻏", "飠"
        ));
        values.put("8", List.of(
            "青", "其", "雨", "非", "齿", "黾", "隹", "阜", "金", "鱼", "隶", "⻗",
            "靑", "齒", "黽", "魚", "隸", "⻖", "雔", "釒"
        ));
        values.put("9", List.of(
            "革", "面", "韭", "骨", "香", "鬼", "食", "音", "首", "韋", "靣", "甶",
            "首", "香"
        ));
        values.put("10", List.of(
            "髟", "鬲", "高", "髙", "鬻"
        ));
        values.put("11", List.of(
            "黄", "麻", "鹿", "蔴", "麤"
        ));
        values.put("12", List.of(
            "鼎", "黑", "黍", "黒", "黃"
        ));
        values.put("13", List.of(
            "鼓", "鼠", "鼡"
        ));
        values.put("14", List.of(
            "鼻", "鼽"
        ));
        values.put("17", List.of(
            "龠"
        ));
        return Collections.unmodifiableMap(values);
    }
}
