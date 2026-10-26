package app.msime.android.policy;

import java.util.ArrayList;
import java.util.List;

/**
 * 插件页的列表、详情和社区插件共用的规则：哪些类型在手机上生效、列表行尾的「使用中」标记、偏好里指着已不在的包的选择、删包后偏好怎么改、短语表和指令表的启用顺序，以及失败码对应的句子。
 *
 * <p>规则照共享设置页的 `plugin-catalog-helpers.ts`、`plugin-preferences.ts` 和 `community-helpers.ts` 的 `communityPluginMessage` 移植，只保留手机上会生效的部分：手机播按键音效包（`mode = "keys"`）、读短语表、指令表、辅助码表、单词本和符号集；特效包、音乐包、按键旋律（`mode = "sequence"`）和脚本插件在手机上不生效，列表不列，只报一个数。按键音的开关在 Android 是键盘页「按键反馈」里的本机开关（`KeyboardFeedbackStore`），不是共享偏好的 `plugins.key_sound.enabled`，所以「使用中」看的是那个开关。
 *
 * <p>不依赖 Android 和 org.json，由 JVM 冒烟 `tests/settings/PluginCatalogPolicySmoke.java` 覆盖；读目录、读写偏好和调用 JNI 的那一半在 `home/` 的插件页里。
 */
public final class PluginCatalogPolicy {
    public static final String SOUND = "sound";
    public static final String COMMAND_TABLE = "command_table";
    public static final String PHRASE_TABLE = "phrase_table";
    public static final String HELPCODE = "helpcode";
    public static final String WORDBOOK = "wordbook";
    public static final String SYMBOL_SET = "symbol_set";

    /** 手机上生效的类型，按「我的插件」分组的顺序（共享页 `kindOrder` 去掉特效包和音乐包）。 */
    public static final List<String> PHONE_KINDS =
        List.of(SOUND, COMMAND_TABLE, PHRASE_TABLE, HELPCODE, WORDBOOK, SYMBOL_SET);

    /** `PluginPreferences::MAX_COMMAND_TABLES`、`MAX_PHRASE_TABLES`。 */
    public static final int MAX_TABLES = 16;
    /** 短语表详情最多预览的行数。 */
    public static final int PHRASE_PREVIEW_ROWS = 20;
    /** 单词本详情预览的单词数。 */
    public static final int WORDBOOK_PREVIEW_WORDS = 5;
    /** 符号集详情每组预览的项数。 */
    public static final int SYMBOL_PREVIEW_ITEMS = 16;
    /** `client-core::preferences::DEFAULT_SOUND_PACK`。 */
    public static final String DEFAULT_SOUND_PACK = "default";
    /** `client-core::preferences::DEFAULT_MELODY_PACK`。 */
    public static final String DEFAULT_MELODY_PACK = "twinkle";
    /** client-services `import.rs` 的 `MAX_ARCHIVE_BYTES`：导入的 .zip 复制进暂存目录时到这里就停。 */
    public static final long MAX_ARCHIVE_BYTES = 80L * 1024 * 1024;

    private PluginCatalogPolicy() {}

    /** `msime_client_plugins` 的 `catalog` 里一个包，只取这里要用的字段；`mode` 只有音效包有，其余为空串。 */
    public record Pack(String kind, String id, String name, String version, String license, String author,
            String description, boolean builtin, String mode) {}

    /**
     * 偏好里与插件有关的选择，以及决定它们生不生效的开关。
     *
     * @param keySoundPack `plugins.key_sound.pack`，空值已按 {@link #DEFAULT_SOUND_PACK}
     * @param keySoundOn 本机的按键音开关
     * @param quickPhraseMode `local_modes.quick_phrase`（缺省开）
     * @param commandMode `local_modes.command`（缺省关）
     */
    public record Selection(String keySoundPack, List<String> phraseTables, List<String> commandTables,
            String helpcodeQuanpin, String helpcodeShuangpin, boolean keySoundOn, boolean quickPhraseMode,
            boolean commandMode) {}

    /** 各类插件统一的显示名，与共享页 `kindLabels` 相同；不认识的类型显示「插件」。 */
    public static String kindLabel(String kind) {
        if (kind == null) return "插件";
        return switch (kind) {
            case SOUND -> "音效包";
            case "music" -> "音乐包";
            case COMMAND_TABLE -> "指令表";
            case "effect" -> "特效包";
            case PHRASE_TABLE -> "短语表";
            case HELPCODE -> "辅助码表";
            case WORDBOOK -> "单词本";
            case SYMBOL_SET -> "符号集";
            case "script" -> "脚本插件";
            default -> "插件";
        };
    }

    /** 一个包的类型显示名：旋律音效包是「音效包·旋律」。 */
    public static String packKindLabel(Pack pack) {
        return SOUND.equals(pack.kind()) && "sequence".equals(pack.mode()) ? "音效包·旋律" : kindLabel(pack.kind());
    }

    /** 这个包在手机上会不会生效，也就是「我的插件」列不列它。 */
    public static boolean usedOnPhone(Pack pack) {
        if (!PHONE_KINDS.contains(pack.kind())) return false;
        return !SOUND.equals(pack.kind()) || !"sequence".equals(pack.mode());
    }

    /** 社区插件的这一类在手机上能不能用：列表只列这些。社区条目不带音效包的发声方式，按键旋律要装上以后才知道。 */
    public static boolean communityKindShown(String kind) {
        return PHONE_KINDS.contains(kind);
    }

    /** 已安装、但这台设备用不上的包有几个；内置的按键旋律随安装包带着，不是用户装的，不算。 */
    public static int unusedCount(List<Pack> packs) {
        int count = 0;
        for (Pack pack : packs) if (!usedOnPhone(pack) && !pack.builtin()) count++;
        return count;
    }

    /** 「我的插件」末尾那一行：有用不上的包时说有几个，没有时为 null。 */
    public static String unusedNote(int count) {
        if (count <= 0) return null;
        return "另有 " + count + " 个已安装的插件在这台设备上不生效（特效包、音乐包、按键旋律或脚本插件），它们在电脑上可以使用。";
    }

    /** 列表行的副标题：内置包只说「内置」，其余是版本和作者。 */
    public static String rowDescription(Pack pack) {
        if (pack.builtin()) return "内置";
        StringBuilder text = new StringBuilder(pack.version());
        if (pack.author() != null && !pack.author().isEmpty()) text.append(" · 作者 ").append(pack.author());
        return text.toString();
    }

    /**
     * 列表行尾的标记：选中的按键音效包是「使用中」（按键音没开时说明），启用的短语表、指令表是「已启用 · 第几位」（对应模式关着时说明），辅助码表说用在哪个方案；没有在用时为 null。与共享页 `packMarker` 相同，只是按键音看本机开关。
     */
    public static String marker(Pack pack, Selection selection) {
        switch (pack.kind()) {
            case SOUND -> {
                if ("sequence".equals(pack.mode()) || !pack.id().equals(selection.keySoundPack())) return null;
                return selection.keySoundOn() ? "使用中" : "已选 · 按键音未开";
            }
            case COMMAND_TABLE -> {
                int position = selection.commandTables().indexOf(pack.id());
                if (position < 0) return null;
                return selection.commandMode() ? "已启用 · 第 " + (position + 1) + " 位" : "已启用 · / 指令未开";
            }
            case PHRASE_TABLE -> {
                int position = selection.phraseTables().indexOf(pack.id());
                if (position < 0) return null;
                return selection.quickPhraseMode() ? "已启用 · 第 " + (position + 1) + " 位" : "已启用 · K 模式未开";
            }
            case HELPCODE -> {
                return helpcodeUses(selection, pack.id());
            }
            default -> {
                return null;
            }
        }
    }

    /** 辅助码表包用在哪个方案上：「用于全拼」「用于双拼」「用于全拼和双拼」，都没用时为 null。 */
    public static String helpcodeUses(Selection selection, String id) {
        boolean quanpin = id.equals(selection.helpcodeQuanpin());
        boolean shuangpin = id.equals(selection.helpcodeShuangpin());
        if (quanpin && shuangpin) return "用于全拼和双拼";
        if (quanpin) return "用于全拼";
        if (shuangpin) return "用于双拼";
        return null;
    }

    /** 偏好里指着一个本机没有的包的选择；`mismatched` 表示同 id 的音效包装着，但是按键旋律，不能当按键音效用。 */
    public record Missing(String kind, String id, List<String> uses, boolean mismatched) {}

    /** 指着本机没有的包的选择，每个包一项，按列表顺序；只看手机上生效的选择。目录读成功之后才有意义。 */
    public static List<Missing> missingSelections(Selection selection, List<Pack> packs) {
        List<Missing> missing = new ArrayList<>();
        addMissing(missing, packs, SOUND, selection.keySoundPack(), "当前音效包");
        for (String id : selection.commandTables()) addMissing(missing, packs, COMMAND_TABLE, id, "已启用的指令表");
        for (String id : selection.phraseTables()) addMissing(missing, packs, PHRASE_TABLE, id, "已启用的短语表");
        addMissing(missing, packs, HELPCODE, selection.helpcodeQuanpin(), "全拼辅助码");
        addMissing(missing, packs, HELPCODE, selection.helpcodeShuangpin(), "双拼辅助码");
        return missing;
    }

    private static void addMissing(List<Missing> missing, List<Pack> packs, String kind, String id, String use) {
        if (id == null || id.isEmpty()) return;
        boolean installed = false;
        boolean usable = false;
        for (Pack pack : packs) {
            if (!pack.kind().equals(kind) || !pack.id().equals(id)) continue;
            installed = true;
            if (!SOUND.equals(kind) || !"sequence".equals(pack.mode())) usable = true;
        }
        if (usable) return;
        for (int index = 0; index < missing.size(); index++) {
            Missing entry = missing.get(index);
            if (entry.kind().equals(kind) && entry.id().equals(id)) {
                List<String> uses = new ArrayList<>(entry.uses());
                uses.add(use);
                missing.set(index, new Missing(kind, id, List.copyOf(uses), entry.mismatched()));
                return;
            }
        }
        missing.add(new Missing(kind, id, List.of(use), installed));
    }

    /** 缺失项的标题：已不在的是「（未找到）」，装着但模式不对的是「（不可用）」。 */
    public static String missingTitle(Missing entry) {
        return entry.id() + (entry.mismatched() ? "（不可用）" : "（未找到）");
    }

    /** 缺失项为什么用不了，接在「用作什么」后面。 */
    public static String missingReason(Missing entry) {
        return entry.mismatched() ? "但它是按键旋律，不能用作按键音效" : "已不在本机";
    }

    /** 缺失项列表行的副标题。 */
    public static String missingDescription(Missing entry) {
        return String.join("、", entry.uses()) + "，" + missingReason(entry);
    }

    /** 缺失项唯一的操作：去掉这个选择，与删包时偏好的改法相同。 */
    public static String missingAction(Missing entry) {
        return switch (entry.kind()) {
            case SOUND -> "改回默认";
            case COMMAND_TABLE, PHRASE_TABLE -> "移除";
            case HELPCODE -> "改回原来的方案";
            default -> "不再使用";
        };
    }

    /** 缺失项详情里的说明，与共享页 `MissingPluginView` 相同。 */
    public static String missingNote(Missing entry) {
        String label = kindLabel(entry.kind());
        String uses = String.join("、", entry.uses());
        return entry.mismatched()
            ? "设置里" + uses + "是「" + entry.id() + "」，" + missingReason(entry) + "。可以" + missingAction(entry)
                + "，或者在「我的插件」里打开另一个" + label + "。"
            : "设置里" + uses + "是「" + entry.id() + "」，但这个" + label + "已不在本机，可能被删除或无法载入。可以重新导入它，或者"
                + missingAction(entry) + "。";
    }

    /** 启用或停用一个短语表、指令表后的顺序：启用的排到最后，停用的去掉；与共享页 `setPhraseTable`、`setCommandTable` 相同。 */
    public static List<String> withTable(List<String> tables, String id, boolean enabled) {
        List<String> next = new ArrayList<>(tables.size() + 1);
        for (String table : tables) if (!table.equals(id)) next.add(table);
        if (enabled) next.add(id);
        return List.copyOf(next);
    }

    /** 短语表、指令表「启用」开关的说明：第几位、满了，或启用后排在哪里。 */
    public static String tableSwitchDescription(String kind, List<String> tables, String id) {
        int position = tables.indexOf(id);
        boolean phrase = PHRASE_TABLE.equals(kind);
        if (position >= 0) {
            return "第 " + (position + 1) + " 位。" + (phrase ? "同一编码下靠前的表先列出。" : "同一指令以靠前的表为准。");
        }
        if (tables.size() >= MAX_TABLES) return "最多启用 " + MAX_TABLES + " 个" + (phrase ? "短语表" : "指令表") + "。";
        return phrase ? "启用后排在已启用的短语表之后。" : "启用后排在已启用的指令表之后。";
    }

    /** 这个表还能不能启用：已经启用的总能停用，没启用的要没满。 */
    public static boolean tableSwitchEnabled(List<String> tables, String id) {
        return tables.contains(id) || tables.size() < MAX_TABLES;
    }

    /** 单词本插件在背单词里的词书 id：`client-services::plugins::wordbook_pack::book_id`。 */
    public static String wordbookBookId(String id) {
        return "pack-" + id;
    }

    /** 导入成功后的提示，与共享页相同。 */
    public static String importedNotice(Pack pack) {
        return "已导入" + packKindLabel(pack) + "「" + pack.name() + "」" + pack.version() + "。";
    }

    /** 宿主失败时显示的句子：`code` 是 client-services 的 `PluginFailure`，`detail` 存在时是被拒的那条规则。与共享页 `pluginErrorMessage` 相同。 */
    public static String errorMessage(String code, String detail, String fallback) {
        String reason = detail == null || detail.isEmpty() ? "" : "（" + detail + "）";
        return switch (code == null ? "" : code) {
            case "plugin_invalid" -> "插件不符合要求" + reason + "。";
            case "plugin_archive" -> "压缩包无法读取" + reason + "。";
            case "plugin_unsupported_source" -> "只能导入插件文件夹或 .zip 文件。";
            case "plugin_reserved" -> "这个 id 属于内置插件，不能覆盖或删除。";
            case "plugin_storage" -> "插件目录不可用，请检查数据目录的权限。";
            case "plugin_exists" -> "本机已经有一个同样 id 的插件，没有覆盖它。";
            case "plugin_not_editable" -> "这个插件不能在设置页编辑" + reason + "。";
            case "storage" -> "插件的数据读写失败，请检查数据目录的权限。";
            default -> fallback;
        };
    }

    /** 社区插件的失败：审核类、账号类的 `community_*`，下载包校验的 `plugin_community_*`，其余按插件失败说。与共享页 `communityPluginMessage` 相同。 */
    public static String communityMessage(String code, String detail, boolean publishing) {
        String value = code == null ? "" : code;
        switch (value) {
            case "community_blocked_content", "account_blocked_content":
                return "内容包含不允许发布的词语，请修改后再提交";
            case "community_screening_unavailable", "account_screening_unavailable":
                return "审核服务暂时不可用，请稍后重试";
            case "community_account_banned", "account_banned":
                return "该账号已被封禁，暂时无法使用账号相关功能";
            case "community_forbidden":
                return "下载后才能评分，且不能给自己的作品评分。";
            case "community_unauthorized":
                return "请先登录后执行此操作。";
            case "community_not_found":
                return "作品不存在或已下架。";
            case "community_cancelled":
                return "账号状态已变化，请重新加载。";
            case "community_storage":
                return "无法安全读取登录状态，请检查设备安全设置。";
            case "community_conflict":
                return publishing ? "发布信息已变更，或已达到发布上限（最多 20 个、合计 32 MB）。" : "作品状态已变化，请刷新后重试。";
            case "community_rate_limited":
                return publishing ? "每小时最多发布 10 次，请稍后再试。" : "请求过于频繁，请稍后再试。";
            case "community_invalid":
                return publishing ? "服务器未接受这个插件：请确认名称、说明和包内容符合发布要求。" : "内容无效，请修改后重试。";
            case "plugin_community_too_large":
                return "插件压缩后不能超过 8 MB。";
            case "plugin_community_checksum":
                return "下载的插件已损坏，请重试。";
            case "plugin_community_mismatch":
                return "下载的插件与作品信息不符，已停止安装。";
            case "storage":
                return "无法读写插件目录，请检查数据目录的权限。";
            default:
                return errorMessage(value, detail, "社区暂时不可用，请稍后重试。");
        }
    }

    /** 这次失败是不是没有登录：页面在失败说明旁边给出「登录」。 */
    public static boolean needsSignIn(String code) {
        return "community_unauthorized".equals(code);
    }

    /** 发布时名称的上限（码位），与共享页 `communityPublishFields` 和服务端一致。 */
    public static final int MAX_PUBLISH_NAME = 32;
    /** 发布时说明的上限（码位）。 */
    public static final int MAX_PUBLISH_DESCRIPTION = 280;

    /** 发布的名称去掉首尾空白后是 1 到 32 个字。 */
    public static boolean publishNameValid(String name) {
        String trimmed = PluginEditorPolicy.trim(name);
        return !trimmed.isEmpty() && trimmed.codePointCount(0, trimmed.length()) <= MAX_PUBLISH_NAME;
    }

    /** 发布的说明去掉首尾空白后不超过 280 个字，可以为空。 */
    public static boolean publishDescriptionValid(String description) {
        String trimmed = PluginEditorPolicy.trim(description);
        return trimmed.codePointCount(0, trimmed.length()) <= MAX_PUBLISH_DESCRIPTION;
    }

    /** 包的大小，以 MB 保留一位小数，与共享页 `candidateSkinMegabytes` 相同。 */
    public static String megabytes(long bytes) {
        return String.format(java.util.Locale.ROOT, "%.1f MB", bytes / (1024.0 * 1024.0));
    }
}
