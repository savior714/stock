import "@testing-library/jest-dom/vitest";

import { afterEach, beforeEach } from "vitest";

import { resetBackendClientForTest } from "@/lib/backend/client";
import { resetMockStore } from "@/lib/backend/mock-client";

beforeEach(() => {
  resetBackendClientForTest();
  resetMockStore();
  localStorage.clear();
});

afterEach(() => {
  resetBackendClientForTest();
  resetMockStore();
  localStorage.clear();
});
