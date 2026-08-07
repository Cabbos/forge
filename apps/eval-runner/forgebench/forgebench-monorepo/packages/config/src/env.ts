export function parsePort(value: string | undefined, fallback = 3000): number {
  if (value === undefined) {
    return fallback;
  }
  if (!/^\d+$/u.test(value)) {
    throw new Error("port must be an integer");
  }
  const port = Number(value);
  if (!Number.isSafeInteger(port) || port < 1 || port > 65535) {
    throw new Error("port must be between 1 and 65535");
  }
  return port;
}
