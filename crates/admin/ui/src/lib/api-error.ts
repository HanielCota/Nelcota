/** Shared transport error contract, independent of API requests and i18n. */
export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public code?: string,
    public params?: Record<string, string | number>,
  ) {
    super(message)
  }
}
