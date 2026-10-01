import { useEffect, useRef, useState } from "react";
import { parseVersion } from "./update-manifest";
import { useMountedRef } from "./use-mounted-ref";

export interface UseAppVersionOptions {
  readAppVersion?: () => Promise<string>;
  fallbackVersion: string;
}

/** Reads the host application version while retaining a stable fallback. */
export function useAppVersion({ readAppVersion, fallbackVersion }: UseAppVersionOptions) {
  const [version, setVersion] = useState(fallbackVersion);
  const mounted = useMountedRef();
  const generation = useRef(0);

  useEffect(() => {
    const current = ++generation.current;
    setVersion(fallbackVersion);
    if (readAppVersion) {
      void readAppVersion()
        .then((value) => {
          const parsed = parseVersion(value);
          if (mounted.current && generation.current === current && parsed)
            setVersion(parsed.display);
        })
        .catch(() => undefined);
    }
    return () => {
      if (generation.current === current) generation.current++;
    };
  }, [fallbackVersion, readAppVersion, mounted]);

  return version;
}
