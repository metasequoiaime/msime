import { useEffect, useState } from "react";
import { parseVersion } from "./update-manifest";

export interface UseAppVersionOptions {
  readAppVersion?: () => Promise<string>;
  fallbackVersion: string;
}

/** Reads the host application version while retaining a stable fallback. */
export function useAppVersion({ readAppVersion, fallbackVersion }: UseAppVersionOptions) {
  const [version, setVersion] = useState(fallbackVersion);

  useEffect(() => {
    let active = true;
    setVersion(fallbackVersion);
    if (readAppVersion) {
      void readAppVersion()
        .then((value) => {
          const parsed = parseVersion(value);
          if (active && parsed) setVersion(parsed.display);
        })
        .catch(() => undefined);
    }
    return () => {
      active = false;
    };
  }, [fallbackVersion, readAppVersion]);

  return version;
}
