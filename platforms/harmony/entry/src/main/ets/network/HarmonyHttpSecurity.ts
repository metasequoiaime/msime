/**
 * The API 21 HTTP stack follows redirects by default. A Location response must therefore abort
 * the request while the first response is still being delivered, before the native stack can send
 * the credentials or body to the next URL.
 */
export interface HttpRequestLike {
  on(type: "headersReceive", callback: (headers: Object) => void): void;
  destroy(): void;
}

export function installNoRedirectGuard(request: HttpRequestLike, onRedirect?: () => void): void {
  request.on("headersReceive", (headers: Object): void => {
    const values: Record<string, Object> = headers as Record<string, Object>;
    for (const key of Object.keys(values)) {
      if (key.toLowerCase() === "location") {
        onRedirect?.();
        request.destroy();
        return;
      }
    }
  });
}

/** `maxRedirects` is understood by newer stacks; the header guard covers API 21. */
export function noRedirectOptions<T extends Object>(options: T): T & { maxRedirects: number } {
  const values: Record<string, Object> = options as unknown as Record<string, Object>;
  values.maxRedirects = 0;
  return options as T & { maxRedirects: number };
}
