package app.msime.android;

import java.util.Map;

/** 社区卡片状态与本机已获取列表的对账规则。 */
public final class CommunityActionPolicy {
    private CommunityActionPolicy() {}

    /** 本机读取结果中不存在的旧状态从表中移除，正在进行的操作除外。 */
    public static <T> void removeStale(Map<String, T> current, Map<String, T> values, T busy) {
        current.entrySet().removeIf(entry -> !values.containsKey(entry.getKey())
            && !busy.equals(entry.getValue()));
    }
}
