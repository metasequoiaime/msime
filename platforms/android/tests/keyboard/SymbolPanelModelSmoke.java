import app.msime.android.SymbolPanelModel;

public final class SymbolPanelModelSmoke {
    public static void main(String[] args) {
        if (SymbolPanelModel.categories().size() != 5) throw new AssertionError("categories");
        if (SymbolPanelModel.categories().get(0).symbols().size() != 30) throw new AssertionError("common");
        if (SymbolPanelModel.categories().get(4).symbols().get(20).equals("http://") == false)
            throw new AssertionError("network shortcuts");
        if (SymbolPanelModel.closesAfterInsert(false) == false
                || SymbolPanelModel.closesAfterInsert(true)) throw new AssertionError("lock");
        System.out.println("SymbolPanelModelSmoke: PASS");
    }
}
