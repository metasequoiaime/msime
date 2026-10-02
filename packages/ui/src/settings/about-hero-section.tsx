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
