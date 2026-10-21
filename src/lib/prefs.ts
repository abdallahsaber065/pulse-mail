// UI-only preferences persisted in localStorage (no secrets, no mail data).
import { createSignal } from "solid-js";

export type Density = "compact" | "default" | "comfortable";

const DENSITY_KEY = "pulse.density";
const ROW_HEIGHTS: Record<Density, number> = { compact: 32, default: 40, comfortable: 52 };

function readDensity(): Density {
  const v = localStorage.getItem(DENSITY_KEY);
  return v === "compact" || v === "comfortable" ? v : "default";
}

const [density, setDensitySignal] = createSignal<Density>(readDensity());

export const prefs = {
  density,
  rowHeight: () => ROW_HEIGHTS[density()],
  setDensity(d: Density) {
    localStorage.setItem(DENSITY_KEY, d);
    setDensitySignal(d);
  },
};
