import type { ComponentPropsWithoutRef, ReactNode } from "react";

export interface SettingsLayoutBlockProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** 设置页布局容器共用属性转发和样式类名拼接。 */
export function SettingsLayoutBlock({
  baseClass,
  children,
  className,
  ...props
}: SettingsLayoutBlockProps & { baseClass: string }) {
  return (
    <div {...props} className={`${baseClass}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
