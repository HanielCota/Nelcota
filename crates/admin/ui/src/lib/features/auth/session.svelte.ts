// Admin session: `undefined` = checking, `null` = signed out.
export const session = $state<{ email: string | null | undefined }>({ email: undefined })
