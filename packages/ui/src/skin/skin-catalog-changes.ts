// 外部皮肤目录在本窗口里被改动的通知。社区图库装入皮肤、云端同步下载或删除皮肤之后发出，主题页据此重新扫描，不必让用户再点「刷新皮肤」。
// 设置页和社区页同在一个 webview 里，用一个模块级的监听集合就够了，不需要放进 React 状态。

const listeners = new Set<() => void>();

/** 告诉正在列出外部皮肤目录的界面：目录内容变了，应当重新扫描。 */
export function notifySkinCatalogChanged(): void {
  for (const listener of listeners) listener();
}

/** 订阅外部皮肤目录的改动通知，返回取消订阅的函数。 */
export function subscribeSkinCatalogChanges(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
