import app.msime.android.CandidateManagementAction;
import app.msime.android.CandidateTranslationPolicy;
import java.util.Arrays;
import java.util.List;

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
        // 「加入生词本」排在所有词库操作后面，id 不与释义（2000 起）和词库操作相撞，点它不会被当成别的操作。
        int item = CandidateManagementAction.PERSONAL_WORDBOOK_MENU_ITEM;
        for (CandidateManagementAction action : actions) {
            check(action.menuItemId() != item);
            check(action.ordinal() < CandidateManagementAction.PERSONAL_WORDBOOK_ORDER);
        }
        check(item >= 2000 + CandidateTranslationPolicy.MAX_INSERTION_SENSES || item < 2000);
        rejects(() -> CandidateManagementAction.fromMenuItemId(item));
        check("加入生词本".equals(CandidateManagementAction.PERSONAL_WORDBOOK_TITLE));
        // 只在有可上屏的释义时出现。
        check(CandidateManagementAction.offersPersonalWordbook(
            CandidateTranslationPolicy.insertionGlosses("interview; audition")));
        check(!CandidateManagementAction.offersPersonalWordbook(CandidateTranslationPolicy.insertionGlosses("")));
        check(!CandidateManagementAction.offersPersonalWordbook(List.of()));
        check(!CandidateManagementAction.offersPersonalWordbook(null));
        // 「朗读」紧挨在「加入生词本」前面、所有词库操作后面，id 同样不撞；只在有能读的英文、本机又装着语音引擎时出现。
        int readAloud = CandidateManagementAction.READ_ALOUD_MENU_ITEM;
        check(readAloud != item);
        check(readAloud >= 2000 + CandidateTranslationPolicy.MAX_INSERTION_SENSES || readAloud < 2000);
        for (CandidateManagementAction action : actions) {
            check(action.menuItemId() != readAloud);
            check(action.ordinal() < CandidateManagementAction.READ_ALOUD_ORDER);
        }
        check(CandidateManagementAction.READ_ALOUD_ORDER < CandidateManagementAction.PERSONAL_WORDBOOK_ORDER);
        rejects(() -> CandidateManagementAction.fromMenuItemId(readAloud));
        check("朗读".equals(CandidateManagementAction.READ_ALOUD_TITLE));
        check(CandidateManagementAction.offersReadAloud("interview", true));
        check(!CandidateManagementAction.offersReadAloud("interview", false));
        check(!CandidateManagementAction.offersReadAloud(null, true));
        check(!CandidateManagementAction.offersReadAloud("", true));
        System.out.println("Android candidate management: menu order, action mapping, position bounds, 朗读 and 加入生词本 placement passed");
    }
}
