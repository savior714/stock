"use client";

import React, { useCallback, useEffect, useState } from "react";

import { parseThemeValue } from "./scanner-utils";

const THEME_STORAGE_KEY = "stock-theme";

export type ThemeMode = "light" | "dark" | "system";

function getInitialTheme(): ThemeMode {
  if (typeof window === "undefined") return "light";
  return parseThemeValue(localStorage.getItem(THEME_STORAGE_KEY));
}

function applyTheme(theme: ThemeMode, mql: MediaQueryList | null) {
  if (theme === "system" && mql) {
    document.documentElement.dataset.theme = mql.matches ? "dark" : "light";
  } else {
    document.documentElement.dataset.theme = theme;
  }
}

export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(getInitialTheme);
  const [mql, setMql] = useState<MediaQueryList | null>(null);

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    setMql(mq);
    /* eslint-enable react-hooks/set-state-in-effect */
    applyTheme(theme, mq);

    const handler = () => applyTheme(theme, mq);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  const setTheme = useCallback(
    (next: ThemeMode) => {
      setThemeState(next);
      localStorage.setItem(THEME_STORAGE_KEY, next);
      const current = mql || window.matchMedia("(prefers-color-scheme: dark)");
      applyTheme(next, current);
    },
    [mql],
  );

  return { theme, setTheme };
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const { theme, setTheme } = useTheme();
  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

const ThemeContext = React.createContext<{
  theme: ThemeMode;
  setTheme: (t: ThemeMode) => void;
}>({ theme: "light", setTheme: () => {} });

export { ThemeContext };
