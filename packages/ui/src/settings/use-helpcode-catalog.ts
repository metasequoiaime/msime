import { useEffect, useState } from "react";
import type { CustomHelpcodeSchema, HelpcodePackOption } from "./pages/helpcode-page";
import type { PluginClient } from "./plugin-types";
import { useAsyncGeneration } from "./use-async-generation";

/** 宿主资源目录里找到的自定义辅助码表；宿主不支持扫描或读取失败时为空。 */
export function useCustomHelpcodeSchemas(
  reader: (() => Promise<CustomHelpcodeSchema[]>) | undefined,
): CustomHelpcodeSchema[] {
  const [schemas, setSchemas] = useState<CustomHelpcodeSchema[]>([]);
  const generation = useAsyncGeneration(reader);
  useEffect(() => {
    const current = generation.current;
    if (!reader) {
      setSchemas([]);
      return;
    }
    void reader()
      .then((next) => {
        if (generation.current === current) setSchemas(next);
      })
      .catch(() => {
        if (generation.current === current) setSchemas([]);
      });
  }, [reader, generation]);
  return schemas;
}

/** 已安装的辅助码表插件，每次进入「输入」页时重读，在别处导入或删除的包下次进来就能看到。 */
export function useHelpcodePacks(
  catalog: PluginClient["catalog"] | undefined,
  inputPageOpen: boolean,
): HelpcodePackOption[] {
  const [packs, setPacks] = useState<HelpcodePackOption[]>([]);
  const generation = useAsyncGeneration(catalog, inputPageOpen);
  useEffect(() => {
    if (!catalog || !inputPageOpen) {
      setPacks([]);
      return;
    }
    const current = generation.current;
    void catalog()
      .then((next) => {
        if (generation.current !== current) return;
        setPacks(
          next.packages
            .filter((pack) => pack.kind === "helpcode")
            .map((pack) => ({ id: pack.id, name: pack.name })),
        );
      })
      .catch(() => {
        if (generation.current === current) setPacks([]);
      });
  }, [catalog, inputPageOpen, generation]);
  return packs;
}
