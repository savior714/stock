import type { RenderOptions } from "@testing-library/react";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const CONSOLE_METHODS = [
  "log",
  "warn",
  "error",
  "info",
  "debug",
  "table",
  "trace",
] as const;

const allLoggers: unknown[] = [];

function captureConsole(): void {
  for (const level of CONSOLE_METHODS) {
    const original = console[level];
    allLoggers.push(original);
    console[level] = (...args: unknown[]) => {
      (original as (...args: unknown[]) => void)(...args);
    };
  }
}

function restoreConsole(): void {
  for (const level of CONSOLE_METHODS) {
    const original = allLoggers.pop();
    if (original) {
      console[level] = original as typeof console.log;
    }
  }
}

export * from "@testing-library/react";
export { render, userEvent, captureConsole, restoreConsole };
