import { MsimeMark } from "../core/brand-logo";
import * as doc from "./document-style";

export interface AboutHeroSectionProps {
  logo: string;
  description: string;
}

/** 关于页的品牌页首，桌面和触屏宿主共用。它放在一个无标题的组里，由组提供底色和圆角，自身不再画一层卡片。 */
export function AboutHeroSection({ logo, description }: AboutHeroSectionProps) {
  return (
    <div className={doc.hero}>
      <div className={doc.mark}>
        <img src={logo} alt="水杉 IME" />
      </div>
      <div>
        <div className={doc.eyebrow}>Metasequoia IME</div>
        <div className={doc.heroTitle}>水杉 IME</div>
        <p>{description}</p>
      </div>
    </div>
  );
}

/** 检查更新所处的状态，与 HarmonyOS 页首胶囊的显示一致：未运行（或运行结果不是 "current"）、运行中、已完成且为最新。 */
export type AboutUpdateState = "idle" | "checking" | "latest";

export interface HarmonyAboutHeroProps {
  /** 当前运行的应用版本，显示为 `版本 {version} · {platformLabel}`。 */
  version: string;
  /** 手机上为 `HarmonyOS`，2in1 上为 `HarmonyOS 2in1`。 */
  platformLabel: string;
  update: AboutUpdateState;
  /** 上次检查结果不是 "current" 时的说明（有新版本、没有发布版本、检查失败），显示在版权信息下方。 */
  updateStatus?: string;
  /** 执行检查更新；不传时不绘制胶囊，用于没有检查更新功能的宿主。 */
  onCheckForUpdate?: () => void;
}

const updateLabels: Record<AboutUpdateState, string> = {
  idle: "检查更新",
  checking: "正在检查…",
  latest: "✓ 已是最新版本",
};

/**
 * HarmonyOS「关于」页头（手机和 2in1）：强调色圆形里的标志、产品名、版本行和版权信息，检查更新以强调色胶囊放在尾侧。与 `AboutHeroSection` 一样，它放在一个无标题分组里，由该分组提供卡片。
 */
export function HarmonyAboutHero({
  version,
  platformLabel,
  update,
  updateStatus,
  onCheckForUpdate,
}: HarmonyAboutHeroProps) {
  return (
    <div className={doc.appHero}>
      <span className={doc.appHeroCircle} aria-hidden="true">
        <MsimeMark size={46} />
      </span>
      <div className={doc.appHeroText}>
        <span className={doc.appHeroTitle}>水杉输入法</span>
        <span className={doc.appHeroVersion}>
          版本 {version} · {platformLabel}
        </span>
        <span className={doc.appHeroCopyright}>© 2026 Metasequoia</span>
        {updateStatus && (
          <p className={doc.appHeroStatus} role="status">
            {updateStatus}
          </p>
        )}
      </div>
      {onCheckForUpdate && (
        <button
          type="button"
          className={doc.updatePill(update === "latest")}
          disabled={update === "checking"}
          aria-busy={update === "checking"}
          onClick={onCheckForUpdate}
        >
          {updateLabels[update]}
        </button>
      )}
    </div>
  );
}
