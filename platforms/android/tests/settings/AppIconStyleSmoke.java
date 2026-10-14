import app.msime.android.AppIconStyle;
import java.util.ArrayList;
import java.util.List;

public final class AppIconStyleSmoke {
    // 组件类名取命名空间，不能取可能带产品版本后缀的 applicationId。
    private static final String NAMESPACE = "app.msime.android";
    private static final String HOST = NAMESPACE + ".home.HomeActivity";

    public static void main(String[] args) {
        List<AppIconStyle> all = AppIconStyle.all();
        check(all.size() == 5, "five icon styles");
        check(all.get(0) == AppIconStyle.CLASSIC, "classic is first");
        List<String> ids = new ArrayList<>();
        List<String> components = new ArrayList<>();
        for (AppIconStyle style : all) {
            check(!style.id().isEmpty() && !style.title().isEmpty()
                && !style.description().isEmpty(), "every style is named");
            check(!ids.contains(style.id()), "no duplicate id");
            ids.add(style.id());
            String component = style.component();
            check(!style.alias().isEmpty(), "every style has an alias");
            check(!HOST.equals(component), "the real host is never an icon component");
            check(!components.contains(component), "no component is shared");
            components.add(component);
        }
        check((NAMESPACE + ".MainActivityClassic").equals(AppIconStyle.CLASSIC.component()),
            "classic has a dedicated launcher alias");
        check((NAMESPACE + ".MainActivityForest").equals(AppIconStyle.FOREST.component()),
            "the existing forest alias remains compatible");
        check(NAMESPACE.equals(AppIconStyle.namespace()), "manifest namespace");
        check(AppIconStyle.from("sky") == AppIconStyle.SKY, "stored style");
        check(AppIconStyle.from("synthetic-unknown") == AppIconStyle.CLASSIC, "unknown style");
        check(AppIconStyle.from(null) == AppIconStyle.CLASSIC, "missing style");
        System.out.println("App icon style checks passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
