export interface ComposerChip {
  id: string;
  type: "file" | "command";
  value: string;
}

export type ComposerMenuMode = "@" | "/" | null;
