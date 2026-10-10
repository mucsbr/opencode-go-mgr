import type { MessageKey } from "../i18n/index.ts";

export const ACCOUNT_REFRESH_TIMEOUT_MS = 90_000;
export const ACCOUNT_REFRESH_ERROR_KEYS: Record<"timeout", MessageKey> = {
  timeout: "额度刷新超时，请稍后重试",
};

export class AccountRefreshTimeoutError extends Error {
  readonly code = "timeout" as const;
  constructor() {
    super("account_refresh_timeout");
    this.name = "AccountRefreshTimeoutError";
  }
}

/** Bound both queueing and I/O, even if a transport ignores cancellation. */
export async function withAccountRefreshDeadline<T>(
  work: (signal: AbortSignal) => Promise<T>,
  signal?: AbortSignal,
  timeoutMs = ACCOUNT_REFRESH_TIMEOUT_MS,
): Promise<T> {
  const controller = new AbortController();
  let cancel!: () => void;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const stopped = new Promise<never>((_resolve, reject) => {
    const stop = (reason: unknown) => { reject(reason); controller.abort(reason); };
    cancel = () => stop(signal?.reason ?? new DOMException("Aborted", "AbortError"));
    if (signal?.aborted) cancel();
    else signal?.addEventListener("abort", cancel, { once: true });
    timer = setTimeout(() => stop(new AccountRefreshTimeoutError()), timeoutMs);
  });
  try {
    const request = Promise.resolve().then(() => {
      controller.signal.throwIfAborted();
      return work(controller.signal);
    });
    return await Promise.race([request, stopped]);
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", cancel);
  }
}
