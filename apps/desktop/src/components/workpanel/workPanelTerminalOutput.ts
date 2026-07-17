type SanitizerState = "text" | "escape" | "csi" | "osc" | "oscEscape";

export function createTerminalOutputSanitizer() {
  let state: SanitizerState = "text";
  let pendingCarriageReturn = false;

  return {
    push(chunk: string) {
      let output = "";

      for (const character of chunk) {
        if (pendingCarriageReturn) {
          if (character === "\n") {
            output += "\r\n";
            pendingCarriageReturn = false;
            continue;
          }
          output += "\n";
          pendingCarriageReturn = false;
        }

        if (state === "text") {
          if (character === "\u001b") {
            state = "escape";
          } else if (character === "\r") {
            pendingCarriageReturn = true;
          } else {
            output += character;
          }
          continue;
        }

        if (state === "escape") {
          if (character === "[") {
            state = "csi";
          } else if (character === "]") {
            state = "osc";
          } else if (character === "\u001b") {
            state = "escape";
          } else {
            state = "text";
            if (character === "\r") pendingCarriageReturn = true;
            else output += character;
          }
          continue;
        }

        if (state === "csi") {
          if (character >= "@" && character <= "~") state = "text";
          continue;
        }

        if (state === "osc") {
          if (character === "\u0007") state = "text";
          else if (character === "\u001b") state = "oscEscape";
          continue;
        }

        if (character === "\\") state = "text";
        else if (character !== "\u001b") state = "osc";
      }

      return output;
    },
    reset() {
      state = "text";
      pendingCarriageReturn = false;
    },
  };
}
