"use client";

import React, { useCallback, useEffect, useState } from "react";

import { parseThemeValue } from "./scanner-utils";

const THEME_STORAGE_KEY = "stock-theme";

export type ThemeMode = "light" | "dark" | "system";

export function resolveTheme(mode: ThemeMode, mql: MediaQueryList | null): "light" | "dark" {
  if (mode === "system") {
    return mql && mql.matches ? "dark" : "light";
  }
  return mode;
}

function getInitialTheme(): ThemeMode {
  if (typeof window === "undefined") return "light";
  return parseThemeValue(localStorage.getItem(THEME_STORAGE_KEY));
}

function applyResolvedTheme(resolved: "light" | "dark"): void {
  document.documentElement.dataset.theme = resolved;
}

export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(getInitialTheme);
  const [mql, setMql] = useState<MediaQueryList | null>(null);

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    setMql(mq);
    /* eslint-enable react-hooks/set-state-in-effect */
  }, []);

  useEffect(() => {
    const resolved = resolveTheme(theme, mql);
    applyResolvedTheme(resolved);
  }, [theme, mql]);

  const setTheme = useCallback(
    (next: ThemeMode) => {
      setThemeState(next);
      localStorage.setItem(THEME_STORAGE_KEY, next);
    },
    [],
  );

  return { theme, setTheme };
}

export function useThemeContext() {
  return useContext(ThemeContext);
}

function useContext<C>(ctx: React.Context<C>): C {
  const context = React.useContext(ctx);
  if (!context) {
    throw new Error("useContext must be used within a ThemeProvider");
  }
  return context;
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
