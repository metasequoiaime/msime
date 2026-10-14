// 全屏呈现以页面自身的背景覆盖整个设置界面，并避开系统栏。功能页里打开的社区图库（`CommunityGallerySubview`）也用这一套。
export const fullScreenSurface =
  "fixed inset-0 z-40 overflow-y-auto bg-[var(--p-bg)] pt-[env(safe-area-inset-top,0px)] pb-[env(safe-area-inset-bottom,0px)] text-[var(--p-text)] [font-family:var(--p-font)]";
export const fullScreenColumn = "mx-auto flex w-full max-w-[760px] flex-col gap-3.5 px-4 pt-1 pb-6";
