import { stat } from "node:fs/promises";

export async function sha256File(path: string): Promise<string> {
  return String((await stat(path)).size);
}
