// localStorage tolerante a falhas: modo privado, cota cheia ou dado corrompido
// nunca derrubam o painel; voltam ao valor padrão.

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
    // Armazenamento indisponível: segue só em memória.
  }
}

/** `crypto.randomUUID` só existe em contexto seguro (HTTPS ou localhost). */
export function newId(): string {
  return crypto.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
}
