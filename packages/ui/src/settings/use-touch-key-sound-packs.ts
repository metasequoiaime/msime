import { useEffect, useState } from "react";
import { soundPackUse } from "./plugin-catalog-helpers";
import type { PluginClient, PluginPackage } from "./plugin-types";
import { useAsyncGeneration } from "./use-async-generation";

/** 目录里能当按键音的音效包：`kind` 为 `sound` 且按键样本模式（没写模式的包按按键包算，与宿主读法一致），按目录给出的顺序。 */
export function touchKeySoundPacks(packages: readonly PluginPackage[]): PluginPackage[] {
  return packages.filter((pack) => pack.kind === "sound" && soundPackUse(pack) === "keys");
}

/**
 * 触屏「按键反馈」组「音效」行可选的音效包，每次进入页面时重读目录，别处导入或删除的包下次进来就能看到。没有目录（宿主不报 `touch_key_sound_packs` 时调用方传 undefined）、还没读到或读取失败时为 undefined，调用方据此不画这一行。
 */
export function useTouchKeySoundPacks(
  catalog: PluginClient["catalog"] | undefined,
  pageOpen: boolean,
): PluginPackage[] | undefined {
  const [packs, setPacks] = useState<PluginPackage[]>();
  const generation = useAsyncGeneration(catalog, pageOpen);
  useEffect(() => {
    if (!catalog || !pageOpen) {
      setPacks(undefined);
      return;
    }
    const current = generation.current;
    void catalog()
      .then((next) => {
        if (generation.current === current) setPacks(touchKeySoundPacks(next.packages));
      })
      .catch(() => {
        if (generation.current === current) setPacks(undefined);
      });
  }, [catalog, pageOpen, generation]);
  return packs;
}
