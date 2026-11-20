import type { AppError, ErrorCode } from "@/lib/bindings";
import { format, strings } from "@/i18n";

/** Narrows an unknown rejection (from a command) to the Rust `AppError` shape. */
export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  const code = v["code"] as Record<string, unknown> | null | undefined;
  return (
    typeof v["message"] === "string" &&
    typeof v["retryable"] === "boolean" &&
    typeof code === "object" &&
    code !== null &&
    typeof code["type"] === "string"
  );
}

/** User-facing message for an error code: what happened + the one next step (spec §9). */
export function messageForCode(code: ErrorCode): string {
  const e = strings.errors;
  switch (code.type) {
    case "rateLimited":
      return format(e.rateLimited, { seconds: code.retryAfter });
    case "upstream":
      return format(e.upstream, { status: code.status });
    case "extensionUnsupportedFeature":
      return format(e.extensionUnsupportedFeature, { extension: code.extension, feature: code.feature });
    default:
      return e[code.type];
  }
}

/** User-facing message for anything a command may reject with. */
export function errorMessage(error: unknown): string {
  return isAppError(error) ? messageForCode(error.code) : strings.errors.unknown;
}
