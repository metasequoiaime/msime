import * as doc from "./document-style";

export interface AboutHeroSectionProps {
  logo: string;
  description: string;
}

/** Brand header shared by the about page on desktop and mobile hosts. */
export function AboutHeroSection({ logo, description }: AboutHeroSectionProps) {
  return (
    <div className={`section ${doc.hero}`}>
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
