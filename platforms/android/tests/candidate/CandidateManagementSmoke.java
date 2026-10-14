import app.msime.android.CandidateManagementAction;
import java.util.Arrays;

public final class CandidateManagementSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    static void rejects(Runnable operation) {
        try {
            operation.run();
            throw new AssertionError("Expected invalid candidate-management value to be rejected");
        } catch (IllegalArgumentException | IllegalStateException expected) {
            // Expected.
        }
    }

    public static void main(String[] args) {
        CandidateManagementAction[] actions = CandidateManagementAction.values();
        check(Arrays.equals(Arrays.stream(actions).map(CandidateManagementAction::title).toArray(),
            new Object[] { "优先显示", "固定到首位", "取消固定", "删除词条…" }));
        for (CandidateManagementAction action : actions)
            check(CandidateManagementAction.fromMenuItemId(action.menuItemId()) == action);
        check(CandidateManagementAction.FIX_FIRST.fixedPosition() == 1);
        check(CandidateManagementAction.validatePosition(1) == 1);
        check(CandidateManagementAction.validatePosition(5) == 5);
        check(CandidateManagementAction.REMOVE.confirmationRequired());
        check(!CandidateManagementAction.PROMOTE.confirmationRequired());
        // 删除的确认收在子菜单里，与 iOS 的「确认删除此词条」同款文字；别的操作没有这一项。
        check("确认删除此词条".equals(CandidateManagementAction.REMOVE.confirmationTitle()));
        rejects(CandidateManagementAction.PROMOTE::confirmationTitle);
        rejects(() -> CandidateManagementAction.validatePosition(0));
        rejects(() -> CandidateManagementAction.validatePosition(6));
        rejects(() -> CandidateManagementAction.fromMenuItemId(-1));
        rejects(CandidateManagementAction.PROMOTE::fixedPosition);
        System.out.println("Android candidate management: menu order, action mapping and position bounds passed");
    }
}
