"use client";

import { createContext, useContext, useEffect, useState, ReactNode } from "react";

type Theme = "light" | "dark";

interface ThemeCtx { theme: Theme; toggle: () => void; }

const Ctx = createContext<ThemeCtx>({ theme: "light", toggle: () => {} });

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>("light");

  useEffect(() => {
    const saved = localStorage.getItem("ubl-theme") as Theme | null;
    const sys   = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    apply(saved ?? sys);
  }, []);

  function apply(t: Theme) {
    setTheme(t);
    document.documentElement.setAttribute("data-theme", t);
    localStorage.setItem("ubl-theme", t);
  }

  return (
    <Ctx.Provider value={{ theme, toggle: () => apply(theme === "light" ? "dark" : "light") }}>
      {children}
    </Ctx.Provider>
  );
}

export const useTheme = () => useContext(Ctx);
