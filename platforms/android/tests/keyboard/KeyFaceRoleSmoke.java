import app.msime.android.KeyFaceRole;
import app.msime.android.KeyFaceSlot;

/** 键组角色表自洽：提示位置都在模板里，领头组只有一层、不指向自己，有意宽窄不一的角色写了原因。 */
public final class KeyFaceRoleSmoke {
    private KeyFaceRoleSmoke() { }

    public static void main(String[] args) {
        for (KeyFaceRole role : KeyFaceRole.values()) {
            check(role.template().has(KeyFaceSlot.MAIN), role + " 的模板没有主标签");
            for (KeyFaceSlot slot : role.slots())
                check(role.template().has(slot), role + " 用到的位置 " + slot + " 不在模板 " + role.template().name() + " 里");
            for (KeyFaceRole leader : role.leaders()) {
                check(leader != role, role + " 不应以自己为上限");
                check(leader.leaders().isEmpty(), role + " 的领头组 " + leader + " 不应再有领头组（上限只有一层）");
            }
            check(role.mixedWidths() == null || !role.mixedWidths().isBlank(), role + " 有意宽窄不一时要写原因");
        }
        // 功能键以这副键盘的字母为上限，字母组本身不设上限。
        check(KeyFaceRole.FUNCTIONS.leaders().equals(KeyFaceRole.KEYBOARD_LETTERS), "功能键应以这副键盘的字母为上限");
        for (KeyFaceRole letters : KeyFaceRole.KEYBOARD_LETTERS) check(letters.leaders().isEmpty(), letters + " 不应另设上限");
        // 14/18 键字母有意宽窄不一；26 键字母、数字行应当一样大。
        check(KeyFaceRole.FOURTEEN_LETTERS.mixedWidths() != null && KeyFaceRole.LETTERS.mixedWidths() == null
            && KeyFaceRole.NUMBER_ROW.mixedWidths() == null, "宽窄声明不对");
        System.out.println("KeyFaceRoleSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
