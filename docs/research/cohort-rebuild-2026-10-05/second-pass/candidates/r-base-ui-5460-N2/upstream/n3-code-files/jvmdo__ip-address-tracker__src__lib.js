import { Mask } from "maska";

export const mask = new Mask({
  mask: (value) => {
    // Break masking for IPv6
    if (value.startsWith(" ")) {
      return;
    }

    // If input looks like an IP (digits and dots only), apply IP mask
    if (/^[\d.]+$/.test(value)) {
      return "#00.#00.#00.#00";
    }

    // Disable mask for domains or URLs
    return;
  },
  tokens: {
    0: { pattern: /[0-9]/, optional: true },
  },
});
