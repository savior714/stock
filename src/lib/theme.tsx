"use client";

import React, { useCallback, useEffect, useState } from "react";

import { parseThemeValue, resolveTheme } from "./scanner-utils";

const THEME_STORAGE_KEY = "stock-theme";

export type ThemeMode = "light" | "dark" | "system";

function getInitialTheme(): ThemeMode {
  if (typeof window === "undefined") return "light";

  try {
    return parseThemeValue(localStorage.getItem(THEME_STORAGE_KEY));
  } catch {
    return "light";
  }
}

function applyResolvedTheme(resolved: "light" | "dark"): void {
  document.documentElement.dataset.theme = resolved;
}

export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(getInitialTheme);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");

    const apply = () => {
      const resolved = resolveTheme(theme, media);
      applyResolvedTheme(resolved);
    };

    apply();
    media.addEventListener("change", apply);

    return () => {
      media.removeEventListener("change", apply);
    };
  }, [theme]);

  const setTheme = useCallback(
    (next: ThemeMode) => {
      setThemeState(next);
      try {
        localStorage.setItem(THEME_STORAGE_KEY, next);
      } catch {
        // storage full or unavailable — theme state already updated
      }
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
