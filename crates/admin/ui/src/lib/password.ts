// Senha aleatória para contas criadas pelo painel. Sem caracteres ambíguos
// (0/O, 1/l/I), para poder ser ditada ou digitada a partir da tela.

const ALPHABET = 'abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789-_'

/** Índice uniforme em [0, n): descarta bytes que causariam viés de módulo. */
function uniform(n: number, random: () => number): number {
  const limit = 256 - (256 % n)
  for (;;) {
    const byte = random()
    if (byte < limit) return byte % n
  }
}

const cryptoByte = () => crypto.getRandomValues(new Uint8Array(1))[0]

/** 20 caracteres de 59 símbolos ≈ 117 bits de entropia. */
export function generatePassword(length = 20, random: () => number = cryptoByte): string {
  return Array.from({ length }, () => ALPHABET[uniform(ALPHABET.length, random)]).join('')
}

/** Mesma regra do servidor (nelcota_auth::validate_password), para avisar antes de enviar. */
export function passwordProblem(password: string): string | null {
  const length = [...password].length
  if (length < 8) return 'mínimo de 8 caracteres'
  if (length > 256) return 'máximo de 256 caracteres'
  return null
}
