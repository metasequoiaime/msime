import { useEffect, useState } from "react";
import { useAsyncGeneration } from "../settings/use-async-generation";

export type CandidateSkinPreviewLoader = (id: string) => Promise<string>;

/** 候选窗口皮肤包的预览图；卡片和详情共用调用方提供的客户端缓存。 */
export function CandidateSkinPreviewImage({
  id,
  name,
  load,
  className,
}: {
  id: string;
  name: string;
  load: CandidateSkinPreviewLoader;
  className: string;
}) {
  const [url, setUrl] = useState("");
  const [failed, setFailed] = useState(false);
  const generation = useAsyncGeneration(id, load);
  useEffect(() => {
    const requestGeneration = generation.current;
    setUrl("");
    setFailed(false);
    void load(id)
      .then((value) => {
        if (generation.current === requestGeneration) setUrl(value);
      })
      .catch(() => {
        if (generation.current === requestGeneration) setFailed(true);
      });
  }, [id, load, generation]);
  return (
    <span className={`${className} grid min-h-[96px] place-items-center`}>
      {url ? (
        <img className="block max-h-[220px] w-full object-contain" src={url} alt={`${name} 预览`} />
      ) : (
        <span className="text-xs [color:var(--p-sub)]">
          {failed ? "预览图加载失败" : "正在读取预览…"}
        </span>
      )}
    </span>
  );
}
