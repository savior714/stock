export type AppErrorCode =
  | "validation"
  | "not_found"
  | "conflict"
  | "database"
  | "provider_rate_limited"
  | "provider_unavailable"
  | "invalid_market_data"
  | "insufficient_data"
  | "cancelled"
  | "internal";

export type AppErrorPayload = {
  code: AppErrorCode;
  message: string;
  detail: string | null;
  retryable: boolean;
};

const FALLBACK: AppErrorPayload = {
  code: "internal",
  message: "알 수 없는 오류가 발생했습니다.",
  detail: null,
  retryable: false,
};

function isAppErrorObject(obj: unknown): obj is Record<string, unknown> & { code: unknown; message: unknown } {
  return (
    obj !== null &&
    typeof obj === "object" &&
    "code" in obj &&
    "message" in obj
  );
}

function tryParseAppError(obj: Record<string, unknown>): AppErrorPayload | null {
  const code = obj.code;
  const message = obj.message;

  if (typeof code !== "string" || typeof message !== "string") {
    return null;
  }

  const knownCodes: AppErrorCode[] = [
    "validation",
    "not_found",
    "conflict",
    "database",
    "provider_rate_limited",
    "provider_unavailable",
    "invalid_market_data",
    "insufficient_data",
    "cancelled",
    "internal",
  ];

  if (!knownCodes.includes(code as AppErrorCode)) {
    return null;
  }

  const detail = typeof obj.detail === "string" ? obj.detail : null;
  const retryable = typeof obj.retryable === "boolean" ? obj.retryable : false;

  return { code: code as AppErrorCode, message, detail, retryable };
}

export function parseAppError(error: unknown): AppErrorPayload {
  // Plain string error
  if (typeof error === "string") {
    if (error === "") {
      return FALLBACK;
    }
    return { code: "internal", message: error, detail: null, retryable: false };
  }

  if (!isAppErrorObject(error)) {
    return FALLBACK;
  }

  // Direct AppError object (code + message already parsed)
  const direct = tryParseAppError(error);
  if (direct) {
    return direct;
  }

  // Tauri invoke error: the AppError JSON is embedded in the `message` field
  if (typeof error.message === "string") {
    try {
      const parsed = JSON.parse(error.message);
      const nested = tryParseAppError(parsed);
      if (nested) {
        return nested;
      }
    } catch {
      // Not JSON, fall through
    }
  }

  // Tauri error with plain message string
  if (typeof error.message === "string" && error.message !== "") {
    return { code: "internal", message: error.message, detail: null, retryable: false };
  }

  return FALLBACK;
}

export function formatAppError(error: unknown): string {
  const payload = parseAppError(error);

  if (payload.detail) {
    return `${payload.message}: ${payload.detail}`;
  }
  return payload.message;
}
