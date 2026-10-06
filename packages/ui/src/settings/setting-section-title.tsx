import type { ReactNode } from "react";

export interface SettingSectionTitleProps {
  title: ReactNode;
  description?: ReactNode;
  as?: "div" | "p" | "span";
  className?: string;
}

/** Shared title markup for the standalone settings field and section layouts. */
export function SettingSectionTitle({
  title,
  description,
  as = "span",
  className = "section-title",
}: SettingSectionTitleProps) {
  const Title = as;
  return (
    <Title className={className}>
      {title}
      {description !== undefined && <small>{description}</small>}
    </Title>
  );
}
