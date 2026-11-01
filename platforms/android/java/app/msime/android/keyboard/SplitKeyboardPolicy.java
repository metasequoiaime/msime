package app.msime.android;

/**
 * 大屏的分离式键盘：什么时候分、哪些布局分、每一行从哪里断开、中间空出多宽，以及它与单手模式的互斥。纯计算，不依赖 Android，便于在 JVM 上测试。
 *
 * <p>分离需要本地设置 `platform.android.split_keyboard` 打开、当前窗口是大屏（{@link KeyboardFormFactorPolicy#expanded}，按 `smallestScreenWidthDp` 判断，手机横屏不算），并且横屏或 `touch_keyboard_split_portrait` 打开；正在画的布局也必须属于 26 键一族（{@link #splitsLayout}）。九键、日文假名网格、手写、大千注音、笔画和注音 9 键这类网格布局不分离，键盘照常画成大屏的 720 dp 居中外框。
 *
 * <p>手机横屏另有一个分支（#10440）：本地设置 `platform.android.split_keyboard_phone_landscape` 打开（默认关、不随账号同步，免得手机继承平板上的开启状态）、横屏、`smallestScreenWidthDp` 低于分离启用宽度，并且按键盘实际可用宽度算出的分离后第一排字母键不窄于 {@link #PHONE_MIN_LETTER_KEY_WIDTH_DP}（{@link #phoneActive}）。可用宽度由调用方给出键盘列扣掉左右系统栏内边距之后的宽度，分屏、侧边导航栏和刘海留白都算在里面，不按屏幕宽度猜（iPhone 上按屏幕宽度判断在真机上从不生效，见 #10342）。手机分离不走大屏的 720 dp 居中外框：{@link KeyboardFormFactorPolicy#surfaceWidthDp} 在分离时总是铺满。
 *
 * <p>每一行都是「不分离时的那一行，整体最多占键盘宽度的 75%，宽窗口里原始键区合计不超过 720 dp，在中间某个键的边界上断开，左右两段各自贴到键盘两侧」。中间至少空出的 25% 是一个不属于任何键的占位视图，键与键之间原有的宽度比例、第二行的半键缩进都原样保留，所以第二行的断口比第一行偏右半个键，就像整副键盘从中间掰开。断点取使左段份额最接近整行一半的键边界；两个边界一样接近时（奇数个等宽键）多出来的那个键归左边。底行的空格键不在键边界上断，而是拆成两个空格键，分别放在两半的内侧，左边那个的宽度让空隙正好居中。
 *
 * <p>字母层的第二、三行在右半边内侧重复左半边最内侧的那个字母（#6022）：a–l 行是 asdfg | ghjkl，⇧ zxcvbnm ⌫ 是 ⇧zxcv | vbnm⌫，双手握持时两边拇指都够得着 G、V，双拼也更顺手。重复的键从空隙里占一个键宽（{@link #gapWeight(float, float)}），所以三行的键宽不变，这两行的空隙比第一行窄一个键；两段的份额恰好相等（第二行 0.5 + 5 对 5 + 0.5，第三行 ⇧ + 4 对 4 + ⌫），空隙仍在正中。只有重复之后两段正好相等时才重复（{@link #duplicatesInnerKey}）：微软双拼显示第十个键 `;` 时第二行没有半键缩进，asdfg | hjkl; 本来就是五对五，再重复 G 右边就多出一个键、空隙偏开半个键，所以这一行不重复。第一行 qwert | yuiop 本来左右各五个，不重复；数字行、123 / #+= 层也不重复。
 */
public final class SplitKeyboardPolicy {
    public static final String PORTRAIT_PREFERENCE = "touch_keyboard_split_portrait";
    /** 中间空隙占整行宽度的最小比例。 */
    public static final float GAP_FRACTION = 0.25f;
    /** 分离阈值的有界设置，默认保持原来的 600 dp。 */
    public static final int MIN_THRESHOLD_DP = 480;
    public static final int MAX_THRESHOLD_DP = 1000;
    public static final int DEFAULT_THRESHOLD_DP = KeyboardFormFactorPolicy.EXPANDED_SMALLEST_WIDTH_DP;
    public static final int THRESHOLD_STEP_DP = 20;
    /** 中间空隙百分比的有界设置，默认保持原来的 25%。 */
    public static final int MIN_GAP_PERCENT = 10;
    public static final int MAX_GAP_PERCENT = 40;
    public static final int DEFAULT_GAP_PERCENT = 25;
    public static final int GAP_PERCENT_STEP = 5;
    /** 两侧原始键区的合计上限；重复 G/V 仍从中缝借一个键宽。 */
    public static final int MAX_KEY_AREA_WIDTH_DP = KeyboardFormFactorPolicy.EXPANDED_SURFACE_MAX_WIDTH_DP;
    /** 拆开的空格键每一半至少保留原宽度的这个比例，避免某一侧只剩一条窄缝。中/英挪到回车左边后，空格右侧的键（。中 ↵）比左侧（123 ，）宽得多，逗号隐藏时要让空隙仍落在正中，右半只剩约 16%，所以下限取 15%。 */
    public static final float MIN_SPACE_SHARE = 0.15f;
    /** 比较份额时的容差：键宽份额是 1.4、1.05 这样的浮点数，相等的两个候选断点不能因为舍入误差分出先后。 */
    private static final float EPSILON = 1e-3f;

    /** 手机横屏分离后字母键（扣掉按键间距的可见宽度）至少要有多宽才分（#10440）。与 iPhone 的 36 pt 相同（#10342）：不比手机竖屏时的字母键窄，只挡住窄得不正常的键盘。 */
    public static final int PHONE_MIN_LETTER_KEY_WIDTH_DP = 36;

    private SplitKeyboardPolicy() {}

    /**
     * 分离式键盘的前提是否满足（与当前布局无关）。
     *
     * @param enabled 本地设置 `platform.android.split_keyboard`
     * @param smallestWidthDp `Configuration.smallestScreenWidthDp`
     * @param landscape `Configuration.orientation` 是否为横屏
     */
    public static boolean active(boolean enabled, int smallestWidthDp, boolean landscape) {
        return active(enabled, smallestWidthDp, landscape, false, DEFAULT_THRESHOLD_DP);
    }

    /** 竖屏覆盖只改变方向条件，不绕过主开关和大屏边界。 */
    public static boolean active(boolean enabled, int smallestWidthDp, boolean landscape, boolean portrait) {
        return active(enabled, smallestWidthDp, landscape, portrait, DEFAULT_THRESHOLD_DP);
    }

    /** 与默认重载相同，但由本机设置提供阈值；非法值按默认值处理。 */
    public static boolean active(boolean enabled, int smallestWidthDp, boolean landscape, boolean portrait, int thresholdDp) {
        int threshold = BoundsPolicy.bounded(thresholdDp, MIN_THRESHOLD_DP, MAX_THRESHOLD_DP);
        return enabled && (landscape || portrait) && smallestWidthDp >= threshold;
    }

    /**
     * 这个界面常量是否画成分离式：标准 26 键（全拼、各家双拼含微软双拼的 ; 键、五笔、英文、日文罗马字、粤拼、越南语、藏文）和韩文两套式。它们的数字符号页（新设计的 123 / #+= 层）跟着字母层一起分离；手写、笔画和注音 9 键虽然也把符号页交给 26 键行，但字母层是网格，符号页也不分离，免得同一个方案在两页之间来回变形。
     */
    public static boolean splitsLayout(int touchLayout) {
        return touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
            || touchLayout == KeyboardLayout.KOREAN_LAYOUT;
    }

    /** 当前是否真的画成分离式：前提满足且正在画的布局可以分离。键盘外框宽度、行的拆分和单手模式的互斥都只看这一个结果。 */
    public static boolean drawn(boolean enabled, int smallestWidthDp, boolean landscape, int touchLayout) {
        return drawn(enabled, smallestWidthDp, landscape, touchLayout, false);
    }

    /** 当前窗口的方向变化不清除偏好；窄外屏仍不分离。 */
    public static boolean drawn(boolean enabled, int smallestWidthDp, boolean landscape, int touchLayout,
                                boolean portrait) {
        return drawn(enabled, smallestWidthDp, landscape, touchLayout, portrait, DEFAULT_THRESHOLD_DP);
    }

    public static boolean drawn(boolean enabled, int smallestWidthDp, boolean landscape, int touchLayout,
                                boolean portrait, int thresholdDp) {
        return active(enabled, smallestWidthDp, landscape, portrait, thresholdDp) && splitsLayout(touchLayout);
    }

    /**
     * 手机横屏分离后第一排字母键的可见宽度（dp）：第一排十个等宽键加一段中缝，中缝按 {@link #gapFraction(int, int)} 占整行，每个键再扣掉按键间距（左右各半个）。第一排是最窄的一排：第二排九个字母两侧有半键缩进，第三排的 ⇧ 和 ⌫ 更宽，按同样的中缝算都更宽。
     *
     * @param availableWidthDp 键盘列扣掉左右系统栏内边距后给键行的宽度；未知（≤ 0）时返回 0，不分
     * @param keySpacingTenths 共享偏好 `touch_key_spacing_tenths`（0.1 dp）
     * @param gapPercent 本地设置 `platform.android.split_keyboard_gap`
     */
    public static float phoneLetterKeyWidthDp(int availableWidthDp, int keySpacingTenths, int gapPercent) {
        if (availableWidthDp <= 0) return 0f;
        float gap = gapFraction(availableWidthDp, gapPercent);
        return availableWidthDp * (1f - gap) / 10f - BoundsPolicy.nonNegative(keySpacingTenths) / 10f;
    }

    /**
     * 手机横屏分离式键盘的前提是否满足（与当前布局无关）：本地开关打开、横屏、`smallestScreenWidthDp` 低于分离启用宽度（够宽的设备走大屏分支），并且分离后字母键不窄于 {@link #PHONE_MIN_LETTER_KEY_WIDTH_DP}。竖屏覆盖 `touch_keyboard_split_portrait` 只对大屏生效，这里不看。
     *
     * @param enabled 本地设置 `platform.android.split_keyboard_phone_landscape`
     * @param thresholdDp 本地设置 `platform.android.split_keyboard_threshold`；非法值按默认值处理
     */
    public static boolean phoneActive(boolean enabled, int smallestWidthDp, boolean landscape, int thresholdDp,
                                      int availableWidthDp, int keySpacingTenths, int gapPercent) {
        int threshold = BoundsPolicy.bounded(thresholdDp, MIN_THRESHOLD_DP, MAX_THRESHOLD_DP);
        return enabled && landscape && smallestWidthDp < threshold
            && phoneLetterKeyWidthDp(availableWidthDp, keySpacingTenths, gapPercent) >= PHONE_MIN_LETTER_KEY_WIDTH_DP;
    }

    /** 手机横屏此刻是否真的画成分离式：{@link #phoneActive} 满足且正在画的布局可以分离（{@link #splitsLayout}）。 */
    public static boolean phoneDrawn(boolean enabled, int smallestWidthDp, boolean landscape, int touchLayout,
                                     int thresholdDp, int availableWidthDp, int keySpacingTenths, int gapPercent) {
        return phoneActive(enabled, smallestWidthDp, landscape, thresholdDp, availableWidthDp, keySpacingTenths,
            gapPercent) && splitsLayout(touchLayout);
    }

    /** 份额合计为 `rowWeight` 的一行需要多大份额的空隙，才能让空隙占插入后整行的 {@link #GAP_FRACTION}。 */
    public static float gapWeight(float rowWeight) {
        return gapWeight(rowWeight, 0f, 0);
    }

    /** 当前窗口的中缝比例：小平板保持原比例，大平板把增加的宽度留在中间。 */
    public static float gapFraction(int availableWidthDp) {
        if (availableWidthDp <= 0) return GAP_FRACTION;
        return Math.max(GAP_FRACTION, 1f - (float) MAX_KEY_AREA_WIDTH_DP / availableWidthDp);
    }

    /** 按用户选择的空隙比例，并保留大屏布局为键区留出的最低比例。 */
    public static float gapFraction(int availableWidthDp, int gapPercent) {
        float configured = BoundsPolicy.bounded(gapPercent, MIN_GAP_PERCENT, MAX_GAP_PERCENT) / 100f;
        return Math.max(configured, availableWidthDp <= 0 ? 0f
            : 1f - (float) MAX_KEY_AREA_WIDTH_DP / availableWidthDp);
    }

    /** 按当前窗口宽度计算中缝，数字、字母、符号和底行共用；重复键从中缝借宽。 */
    public static float gapWeight(float rowWeight, float duplicateWeight, int availableWidthDp) {
        float gap = gapFraction(availableWidthDp);
        return BoundsPolicy.nonNegative(BoundsPolicy.nonNegative(rowWeight) * gap / (1f - gap)
            - BoundsPolicy.nonNegative(duplicateWeight));
    }

    public static float gapWeight(float rowWeight, float duplicateWeight, int availableWidthDp, int gapPercent) {
        float gap = gapFraction(availableWidthDp, gapPercent);
        return BoundsPolicy.nonNegative(BoundsPolicy.nonNegative(rowWeight) * gap / (1f - gap)
            - BoundsPolicy.nonNegative(duplicateWeight));
    }

    /** 空隙里还要放一个份额为 `duplicateWeight` 的重复键时空隙本身的份额：从 {@link #gapWeight(float)} 里扣掉它，整行的总份额不变，键宽也就和不重复的行一样。 */
    public static float gapWeight(float rowWeight, float duplicateWeight) {
        return gapWeight(rowWeight, duplicateWeight, 0);
    }

    /**
     * 分离时字母层的第 `rowIndex` 行（0 起，共 `rowCount` 行字母）要不要在右半边内侧重复左半边最内侧的字母：三行字母的第二、三行（G、V），并且重复之后左右两段的份额正好相等。左段比右段多出的正好是最内侧那个键的份额时才重复；两段本来就相等（微软双拼显示 `;` 的第二行）时不重复，免得空隙偏到一边。
     *
     * @param weights 这一行各子视图按最终可见状态计的份额（同 {@link #cutIndex}），还没有插入空隙
     */
    public static boolean duplicatesInnerKey(int rowIndex, int rowCount, float[] weights) {
        if (rowCount != 3 || (rowIndex != 1 && rowIndex != 2)) return false;
        int cut = cutIndex(weights);
        if (cut <= 0) return false;
        float left = 0f;
        float right = 0f;
        for (int index = 0; index < weights.length; index++) {
            float weight = BoundsPolicy.nonNegative(weights[index]);
            if (index < cut) left += weight;
            else right += weight;
        }
        float inner = BoundsPolicy.nonNegative(weights[cut - 1]);
        return inner > 0f && Math.abs(left - (right + inner)) <= EPSILON;
    }

    /**
     * 一行在哪里断开：返回留在左半边的子视图个数（空隙插在这个下标上）。`weights` 是各子视图按最终可见状态计的份额，隐藏的子视图记 0。
     *
     * <p>取使左段份额最接近整行一半的键边界，一样接近时取左段更多的那个，所以奇数个键时多出的一个归左边：九个键的 a–l 行是 asdfg | hjkl，⇧ zxcvbnm ⌫ 是 ⇧zxcv | bnm⌫。⇧ 与 ⌫ 在行的两端，永远不会被分到同一边以外的地方。
     */
    public static int cutIndex(float[] weights) {
        float total = 0f;
        for (float weight : weights) total += BoundsPolicy.nonNegative(weight);
        float half = total / 2f;
        int best = 0;
        float bestDistance = Float.MAX_VALUE;
        float left = 0f;
        for (int index = 0; index <= weights.length; index++) {
            if (index > 0) left += BoundsPolicy.nonNegative(weights[index - 1]);
            float distance = Math.abs(left - half);
            // 只在严格更近时才换断点，所以一样近时后出现、左段更多的那个边界要单独比一次。
            if (distance < bestDistance - EPSILON
                    || (Math.abs(distance - bestDistance) <= EPSILON && left > half)) {
                best = index;
                bestDistance = distance;
            }
        }
        return best;
    }

    /**
     * 底行的空格键拆成两半时左边那一半的份额：让空隙正好落在整行中间，并且两半各自至少保留 {@link #MIN_SPACE_SHARE} 的原宽度。右边那一半是原份额减去它。
     *
     * @param weights 底行各键按最终可见状态计的份额（空格键按拆开前的整份计）
     * @param spaceIndex 空格键在 `weights` 里的下标
     */
    public static float leftSpaceWeight(float[] weights, int spaceIndex) {
        if (spaceIndex < 0 || spaceIndex >= weights.length)
            throw new IllegalArgumentException("space index " + spaceIndex);
        float total = 0f;
        float before = 0f;
        for (int index = 0; index < weights.length; index++) {
            float weight = BoundsPolicy.nonNegative(weights[index]);
            total += weight;
            if (index < spaceIndex) before += weight;
        }
        float space = BoundsPolicy.nonNegative(weights[spaceIndex]);
        return BoundsPolicy.bounded(total / 2f - before, space * MIN_SPACE_SHARE,
            space * (1f - MIN_SPACE_SHARE));
    }

    /**
     * 分离式键盘画着的时候单手模式不生效：返回实际要套用的单手取值。存着的偏好不改，回到不分离的窗口或布局时原来的单手模式自动回来。
     *
     * @param stored 本地设置 `platform.android.one_handed`（`off` / `left` / `right`）
     * @param splitDrawn {@link #drawn} 的结果
     */
    public static String effectiveOneHanded(String stored, boolean splitDrawn) {
        return splitDrawn || stored == null ? "off" : stored;
    }
}
