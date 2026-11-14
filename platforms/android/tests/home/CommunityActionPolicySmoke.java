package app.msime.android.home;

import app.msime.android.CommunityActionPolicy;
import java.util.HashMap;
import java.util.Map;

/** 社区 Tab 切换后，本机皮肤库变化应清掉旧的已获取状态，同时保留进行中的获取。 */
public final class CommunityActionPolicySmoke {
    private enum Action { AVAILABLE, BUSY, DONE }

    public static void main(String[] args) {
        Map<String, Action> current = new HashMap<>();
        current.put("kept", Action.DONE);
        current.put("removed", Action.DONE);
        current.put("installing", Action.BUSY);

        Map<String, Action> values = Map.of("kept", Action.DONE);
        for (Map.Entry<String, Action> entry : values.entrySet()) {
            if (current.get(entry.getKey()) == Action.BUSY) continue;
            current.put(entry.getKey(), entry.getValue());
        }
        CommunityActionPolicy.removeStale(current, values, Action.BUSY);

        check(current.get("kept") == Action.DONE, "仍在本机库中的皮肤保持已获取");
        check(current.get("removed") == null, "已删除的皮肤清除旧状态");
        check(current.get("installing") == Action.BUSY, "进行中的获取保留忙碌状态");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
