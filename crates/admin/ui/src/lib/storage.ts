// Fault-tolerant localStorage: private mode, a full quota or corrupt data
// never take the panel down; they fall back to the default value.

export function readJson<T>(key: string, parse: (data: unknown) => T | null, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    return raw === null ? fallback : (parse(JSON.parse(raw)) ?? fallback)
  } catch {
    return fallback
  }
}

export function readText(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) ?? fallback
  } catch {
    return fallback
  }
}

export function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value)
  } catch {
    // Storage unavailable: carry on in memory only.
  }
}

/** `crypto.randomUUID` only exists in a secure context (HTTPS or localhost). */
export function newId(): string {
  return crypto.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
}
