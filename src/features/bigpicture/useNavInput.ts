import { useEffect, useRef, useState } from "react";

export type NavAction = "up" | "down" | "left" | "right" | "confirm" | "back" | "favorite";

/** "playstation" when a Sony controller is connected, "xbox" for any other, null without one. */
export type PadStyle = "playstation" | "xbox" | null;

// Standard Gamepad API mapping (https://w3c.github.io/gamepad/#remapping).
const BUTTONS: [number, NavAction][] = [
  [0, "confirm"], // ✕ / A
  [1, "back"], // ○ / B
  [3, "favorite"], // △ / Y
  [12, "up"],
  [13, "down"],
  [14, "left"],
  [15, "right"],
];
const KEYS: Record<string, NavAction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "confirm",
  Escape: "back",
  Backspace: "back",
  f: "favorite",
  F: "favorite",
};
const DIRECTIONS: NavAction[] = ["up", "down", "left", "right"];
const REPEAT_DELAY_MS = 380;
/** A button held while the controller is replaced (DS4Windows switching profile at launch) would
 *  otherwise read as a second press. */
const PRESS_COOLDOWN_MS = 800;
const REPEAT_EVERY_MS = 110;
const STICK_THRESHOLD = 0.55;

/**
 * Keyboard and controller navigation. Controllers are polled once per frame through the browser's
 * Gamepad API — only while this hook is mounted, and the browser pauses frames when Hangar is hidden.
 * Held directions repeat, like on a console menu.
 */
export function useNavInput(onAction: (action: NavAction) => void): PadStyle {
  const handler = useRef(onAction);
  const [style, setStyle] = useState<PadStyle>(null);

  useEffect(() => {
    handler.current = onAction;
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const action = KEYS[e.key];
      if (!action) return;
      e.preventDefault();
      handler.current(action);
    };
    window.addEventListener("keydown", onKey);

    const held = new Map<NavAction, number>(); // action → time of the next repeat
    const fired = new Map<NavAction, number>(); // action → time of its last press
    let frame = 0;
    let lastStyle: PadStyle = null;

    const poll = (now: number) => {
      const pads = navigator.getGamepads().filter((p): p is Gamepad => p !== null && p.connected);
      const pad = pads[0];
      const nextStyle: PadStyle = pad
        ? /054c|sony|dualshock|wireless controller/i.test(pad.id)
          ? "playstation"
          : "xbox"
        : null;
      if (nextStyle !== lastStyle) {
        lastStyle = nextStyle;
        setStyle(nextStyle);
      }

      const pressed = new Set<NavAction>();
      for (const p of pads) {
        for (const [index, action] of BUTTONS) if (p.buttons[index]?.pressed) pressed.add(action);
        const [x = 0, y = 0] = p.axes;
        if (x < -STICK_THRESHOLD) pressed.add("left");
        if (x > STICK_THRESHOLD) pressed.add("right");
        if (y < -STICK_THRESHOLD) pressed.add("up");
        if (y > STICK_THRESHOLD) pressed.add("down");
      }

      for (const action of pressed) {
        const next = held.get(action);
        const last = fired.get(action) ?? -Infinity;
        if (next === undefined) {
          if (DIRECTIONS.includes(action) || now - last >= PRESS_COOLDOWN_MS) {
            handler.current(action);
            fired.set(action, now);
          }
          held.set(action, now + REPEAT_DELAY_MS);
        } else if (DIRECTIONS.includes(action) && now >= next) {
          handler.current(action);
          held.set(action, now + REPEAT_EVERY_MS);
        }
      }
      for (const action of held.keys()) if (!pressed.has(action)) held.delete(action);

      frame = requestAnimationFrame(poll);
    };
    frame = requestAnimationFrame(poll);

    return () => {
      window.removeEventListener("keydown", onKey);
      cancelAnimationFrame(frame);
    };
  }, []);

  return style;
}
