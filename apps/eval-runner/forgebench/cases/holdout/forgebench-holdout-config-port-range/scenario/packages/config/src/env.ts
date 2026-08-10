export function parsePort(value: string | undefined, fallback = 3000): number {
  if (value === undefined) {
    return fallback;
  }
  const port = Number(value);
  if (!Number.isFinite(port) || port < 0 || port > 65536) {
    throw new Error("invalid port");
  }
  return port;
}
