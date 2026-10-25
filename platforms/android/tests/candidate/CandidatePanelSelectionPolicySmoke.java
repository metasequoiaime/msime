import app.msime.android.CandidatePanelSelectionPolicy;

public final class CandidatePanelSelectionPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // #9193：只选掉前面一段、组字还在时面板留着，接着列剩下组字的候选。
        check(CandidatePanelSelectionPolicy.keepsOpen(true, 7, 7, "hao"), "partial selection keeps the panel");
        // 整句上屏或组字清空后没有可选的了，收起。
        check(!CandidatePanelSelectionPolicy.keepsOpen(true, 7, 7, ""), "whole commit closes");
        check(!CandidatePanelSelectionPolicy.keepsOpen(true, 7, 7, null), "missing composition closes");
        // 引擎拒绝（过期的一代）时不重开旧面板。
        check(!CandidatePanelSelectionPolicy.keepsOpen(false, 7, 7, "hao"), "declined selection closes");
        // 选词期间会话停掉或换了（失败恢复、换编辑器），剩下的组字不属于这次选择。
        check(!CandidatePanelSelectionPolicy.keepsOpen(true, 7, 0, "hao"), "stopped session closes");
        check(!CandidatePanelSelectionPolicy.keepsOpen(true, 7, 8, "hao"), "replaced session closes");
        check(!CandidatePanelSelectionPolicy.keepsOpen(true, 0, 0, "hao"), "no session closes");

        // 笔画比的是刚被选掉的那个字，部分选词后清空；没有笔画时不发请求。
        check("".equals(CandidatePanelSelectionPolicy.strokesAfterPartialSelection("hs")), "strokes cleared");
        check(CandidatePanelSelectionPolicy.strokesAfterPartialSelection("") == null, "no strokes, no request");
        check(CandidatePanelSelectionPolicy.strokesAfterPartialSelection(null) == null, "missing strokes");
        System.out.println("Android candidate panel selection: partial picks keep the panel, stale strokes cleared");
    }
}
