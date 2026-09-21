import { useEffect, useState } from "react";

const cache = new Map<string, Promise<string | null>>();

/**
 * The most characteristic colour of an image: pixels are weighted by saturation, so a vivid
 * accent wins over large grey or black areas. Returns "r g b" (for `rgb(var(--x) / a)`), or null.
 */
export function dominantColor(src: string): Promise<string | null> {
  let result = cache.get(src);
  if (!result) {
    result = compute(src);
    cache.set(src, result);
  }
  return result;
}

function compute(src: string): Promise<string | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onerror = () => resolve(null);
    img.onload = () => {
      const w = 24;
      const h = 36;
      const canvas = document.createElement("canvas");
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      if (!ctx) return resolve(null);
      ctx.drawImage(img, 0, 0, w, h);
      const { data } = ctx.getImageData(0, 0, w, h);

      let r = 0;
      let g = 0;
      let b = 0;
      let total = 0;
      for (let i = 0; i < data.length; i += 4) {
        const [pr, pg, pb] = [data[i], data[i + 1], data[i + 2]];
        const max = Math.max(pr, pg, pb);
        const min = Math.min(pr, pg, pb);
        const saturation = max === 0 ? 0 : (max - min) / max;
        const brightness = max / 255;
        // Vivid, mid-bright pixels count most; near-black and washed-out ones barely.
        const weight = saturation * saturation * (brightness > 0.15 ? 1 : 0.1) + 0.002;
        r += pr * weight;
        g += pg * weight;
        b += pb * weight;
        total += weight;
      }
      resolve(`${Math.round(r / total)} ${Math.round(g / total)} ${Math.round(b / total)}`);
    };
    img.src = src;
  });
}

/** Dominant colour of an image as "r g b", once it has been computed. */
export function useDominantColor(src: string | undefined) {
  const [color, setColor] = useState<{ src: string; value: string | null } | null>(null);
  useEffect(() => {
    if (!src) return;
    let cancelled = false;
    dominantColor(src).then((value) => !cancelled && setColor({ src, value }));
    return () => {
      cancelled = true;
    };
  }, [src]);
  return color && color.src === src ? color.value : null;
}
