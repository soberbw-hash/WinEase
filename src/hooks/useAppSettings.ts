import { useCallback, useEffect, useState } from "react";
import type { AppSettings } from "../types";

const STORAGE_KEY = "win-toolbox:settings:v3_2";

const defaultSettings: AppSettings = {
  captureHelperEnabled: false,
};

export function useAppSettings() {
  const [settings, setSettings] = useState<AppSettings>(() => {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return defaultSettings;
    }

    try {
      const stored = JSON.parse(raw) as Partial<AppSettings> | null;
      return {
        captureHelperEnabled: stored?.captureHelperEnabled === true,
      };
    } catch {
      return defaultSettings;
    }
  });

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }, [settings]);

  const updateSettings = useCallback((patch: Partial<AppSettings>) => {
    setSettings((current) => ({ ...current, ...patch }));
  }, []);

  return { settings, updateSettings };
}
