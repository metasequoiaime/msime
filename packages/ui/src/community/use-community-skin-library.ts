import { useEffect, useRef, useState } from "react";
import { deepEqual } from "../core/deep-equal";
import type { Preferences } from "../core/preferences-model";
import type {
  CustomSkinLibraryClient,
  SavedTouchKeyboardSkin,
} from "../keyboard/touch-keyboard-skin-design";
import { updateCustomKeyboard } from "../settings/theme-selection-updates";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { communitySkinMessage } from "./community-helpers";
import type {
  CommunitySkin,
  CommunitySkinClient,
  CommunitySkinTrial,
} from "./community-skin-model";

/** 皮肤库条目按 id 与社区列表项对应，而 id 是 UUID，大小写都可能出现。 */
function skinKey(id: string): string {
  return id.toLowerCase();
}

/** 皮肤卡片上胶囊按钮提供的操作：把设计收进皮肤库，把库里已有的设计用到键盘上，或者因为已在使用而不提供操作。 */
export type CommunitySkinPillState = "get" | "getting" | "use" | "using" | "in-use";

/** 社区卡片的本机皮肤库和获取、使用操作；界面与详情试用由调用方持有。 */
export function useCommunitySkinLibrary({
  client,
  localSkinLibrary,
  harmony,
  browsing,
  preferences,
  onApplyPreferences,
  onSkinApplied,
  toast,
}: {
  client: Pick<CommunitySkinClient, "download" | "finishTrial">;
  localSkinLibrary?: Pick<CustomSkinLibraryClient, "load">;
  harmony: boolean;
  browsing: boolean;
  preferences?: Preferences;
  onApplyPreferences?: (next: Preferences) => void | Promise<void>;
  onSkinApplied?: (id: string) => void;
  toast: (message: string) => void;
}) {
  // 按小写 id 索引的自定义皮肤库。「获取」保存的皮肤以其社区 id 归档，卡片据此改为提供「使用」。
  const [library, setLibrary] = useState<ReadonlyMap<string, SavedTouchKeyboardSkin>>(
    () => new Map(),
  );
  const [pending, setPending] = useState<ReadonlyMap<string, "getting" | "using">>(() => new Map());
  // 同步读取，这样第一次点击尚未渲染时再点一次胶囊按钮不会有任何效果。
  const acting = useRef(new Set<string>());
  // 每次「获取」完成都加一，这样更早开始的皮肤库读取不会丢掉它刚加入的设计。
  const libraryGeneration = useRef(0);
  const takingTrials = useRef(new Map<string, CommunitySkinTrial>());
  useEffect(() => {
    const trials = takingTrials.current;
    return () => {
      for (const unfinished of trials.values()) {
        void client.finishTrial(unfinished.id, false).catch(() => undefined);
      }
      trials.clear();
    };
  }, [client]);
  // 网格重新显示时就重新读取：详情里的「下载并试用」也会把设计收进皮肤库。
  const libraryLoadGeneration = useAsyncGeneration(harmony, browsing, localSkinLibrary);
  useEffect(() => {
    if (!harmony || !browsing || !localSkinLibrary) return;
    const current = libraryLoadGeneration.current;
    const mutationGeneration = libraryGeneration.current;
    void localSkinLibrary.load().then(
      (items) => {
        if (
          libraryLoadGeneration.current !== current ||
          mutationGeneration !== libraryGeneration.current
        )
          return;
        setLibrary(new Map(items.map((item) => [skinKey(item.id), item])));
      },
      // 皮肤库读不出来时所有卡片都停在「获取」，与 Android 一致；再次获取同一设计会按同一 id 重新归档，不会多出一份。
      () => undefined,
    );
  }, [harmony, browsing, localSkinLibrary, libraryLoadGeneration]);

  const pillState = (skin: CommunitySkin): CommunitySkinPillState => {
    const key = skinKey(skin.id);
    const busy = pending.get(key);
    if (busy) return busy;
    const saved = library.get(key);
    if (!saved) return "get";
    return preferences?.global_theme === "custom" &&
      deepEqual(preferences.custom_theme?.keyboard, saved.design)
      ? "in-use"
      : "use";
  };

  const setPendingState = (key: string, state: "getting" | "using" | null) =>
    setPending((current) => {
      const next = new Map(current);
      if (state) next.set(key, state);
      else next.delete(key);
      return next;
    });

  /**
   * 获取：把设计保存进自定义皮肤库，不改变键盘，与 Android 的胶囊按钮一致。
   *
   * 宿主只有一个下载操作，而它做的不止这些：取回设计、用它开始试用，并以社区 id 导入皮肤库，这样服务器会计入这次下载，之后「使用」也能按这个 id 找到设计。结束试用但不保留时会换回之前的皮肤，皮肤库条目保持不动。没能结束的试用留在 `takingTrials` 里，生命周期清理逻辑会在页面离开时重试。
   */
  const take = async (skin: CommunitySkin) => {
    const key = skinKey(skin.id);
    if (acting.current.has(key)) return;
    acting.current.add(key);
    setPendingState(key, "getting");
    try {
      const result = await client.download(skin.id, skin.name);
      takingTrials.current.set(key, result.trial);
      await client.finishTrial(result.trial.id, false);
      takingTrials.current.delete(key);
      libraryGeneration.current += 1;
      setLibrary((current) => new Map(current).set(skinKey(result.skin.id), result.skin));
      toast(`已获取「${skin.name}」，点「使用」换上`);
    } catch (failure) {
      // `communitySkinMessage` 会写明皮肤库最多十二个设计的上限，这是用户能自己处理的失败。
      toast(communitySkinMessage(failure));
    } finally {
      acting.current.delete(key);
      setPendingState(key, null);
    }
  };

  /** 使用：把库里的副本用到键盘上，与在皮肤页选择已保存的设计一样，经由同一项偏好修改。 */
  const apply = async (skin: CommunitySkin) => {
    const key = skinKey(skin.id);
    const saved = library.get(key);
    if (!saved || !preferences || !onApplyPreferences || acting.current.has(key)) return;
    acting.current.add(key);
    setPendingState(key, "using");
    try {
      await onApplyPreferences(updateCustomKeyboard(preferences, saved.design));
      onSkinApplied?.(skin.id);
      toast(`已换上「${skin.name}」`);
    } catch {
      toast("切换失败，保留当前皮肤");
    } finally {
      acting.current.delete(key);
      setPendingState(key, null);
    }
  };

  return { pillState, take, apply };
}
