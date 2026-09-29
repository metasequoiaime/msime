import app.msime.android.AppIconStyle;
import java.util.ArrayList;
import java.util.List;

public final class AppIconStyleSmoke {
    // The namespace the manifest's relative names resolve against. It used to differ from the application id, and every component bug this file guards against came from composing a name out of the application id; both are app.msime.android now, but AppIconStyle still takes the namespace from its own package so the two can differ again without breaking.
    private static final String NAMESPACE = "app.msime.android";
    private static final String LAUNCHER = NAMESPACE + ".home.HomeActivity";

    public static void main(String[] args) {
        List<AppIconStyle> all = AppIconStyle.all();
        check(all.size() == 5, "five icon styles");
        check(all.get(0) == AppIconStyle.CLASSIC, "classic is first, as the shipped default");

        List<String> ids = new ArrayList<>();
        List<String> components = new ArrayList<>();
        for (AppIconStyle style : all) {
            check(!style.id().isEmpty() && !style.title().isEmpty()
                && !style.description().isEmpty(), "every style is named: " + style.id());
            check(!ids.contains(style.id()), "no id appears twice: " + style.id());
            ids.add(style.id());
            String component = style.component(LAUNCHER);
            check(!component.isEmpty(), "every style names a component");
            // Two styles mapping to one component means selecting either leaves the other
            // looking selected, and disabling "the rest" would disable the one just enabled.
            check(!components.contains(component), "no component is shared: " + component);
            components.add(component);
        }

        // Classic is the launcher activity itself: the manifest enables it, and there is no alias
        // to turn on for it. Naming one would disable the real entry point and take the app out of
        // the launcher with no way back from the launcher.
        check(AppIconStyle.CLASSIC.alias().isEmpty(), "classic has no alias of its own");

        // Classic is whatever launcher activity the caller names, never something composed from the
        // application id. Composing them produced `app.msime.android.home.HomeActivity`, a component
        // the package manager has never heard of; disabling it threw, and since every switch
        // disables the styles it is not selecting, every switch failed. The earlier version of this
        // check asserted the composed string, so it agreed with the bug instead of catching it.
        check(LAUNCHER.equals(AppIconStyle.CLASSIC.component(LAUNCHER)),
            "classic is the launcher activity it was given");

        // `.MainActivityX` in the manifest resolves against the namespace, not the application id. While the two differed, composing it with the application id produced a component that did not exist, so the package manager refused every switch.
        check(NAMESPACE.equals(AppIconStyle.namespace()),
            "this class sits at the namespace root the manifest's relative names resolve against");
        check((NAMESPACE + ".MainActivityForest").equals(AppIconStyle.FOREST.component(LAUNCHER)),
            "an alias resolves against the namespace");
        for (AppIconStyle style : all) {
            if (style == AppIconStyle.CLASSIC) continue;
            check(!style.alias().isEmpty(), "every other style has an alias: " + style.id());
        }

        check(AppIconStyle.from("sky") == AppIconStyle.SKY, "a stored id resolves");
        check(AppIconStyle.from("from a newer build") == AppIconStyle.CLASSIC,
            "an unknown id falls back to the component the manifest enables");
        check(AppIconStyle.from(null) == AppIconStyle.CLASSIC, "so does a missing one");
        System.out.println("Android app icon styles: ids, components and fallback passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
