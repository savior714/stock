import type { Metadata } from "next";
import type { ReactNode } from "react";
import "./globals.css";
import { ThemeProvider } from "@/lib/theme";
import MockBadge from "@/components/MockBadge";

export const metadata: Metadata = {
  title: "Stock Scanner",
};

export const viewport = {
  colorScheme: "light dark",
};

export default function RootLayout({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="ko">
      <head>
        <script
          dangerouslySetInnerHTML={{
            __html: `
              (function() {
                try {
                  var stored = localStorage.getItem("stock-theme");
                  var theme = "light";
                  if (stored === "light" || stored === "dark" || stored === "system") {
                    theme = stored;
                  }
                  var resolved = theme === "system"
                    ? window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
                    : theme;
                  document.documentElement.dataset.theme = resolved;
                } catch(e) {}
              })();
            `,
          }}
        />
      </head>
      <body>
        <ThemeProvider>
          {children}
          <MockBadge />
        </ThemeProvider>
      </body>
    </html>
  );
}
