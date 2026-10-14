/** 持有异步监听的取消函数，调用方更换或卸载后释放；未注册时可以没有取消函数。 */
export function trackAsyncSubscription(
  pending: Promise<(() => void) | void>,
  isCurrent: () => boolean,
): () => void {
  let disposed = false;
  let stop: (() => void) | undefined;
  void pending
    .then((unsubscribe) => {
      if (!unsubscribe) return;
      if (disposed || !isCurrent()) unsubscribe();
      else stop = unsubscribe;
    })
    .catch(() => undefined);
  return () => {
    disposed = true;
    stop?.();
    stop = undefined;
  };
}
