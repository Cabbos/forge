export function getQueryErrorMessage(...errors: unknown[]): string {
  const error = errors.find(Boolean);
  if (!error) return "";
  if (error instanceof Error) return error.message;
  return String(error);
}
