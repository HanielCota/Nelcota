// Sessão do admin: `undefined` = verificando, `null` = deslogado.
export const session = $state<{ email: string | null | undefined }>({ email: undefined })
