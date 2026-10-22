// UI-only preferences persisted in localStorage (no secrets, no mail data).
import { createSignal } from "solid-js";
import { PANES, type Pane } from "./pane";

export { effectivePane, nextPane, type Pane } from "./pane";

export type Density = "compact" | "default" | "comfortable";

const DENSITY_KEY = "pulse.density";
const ROW_HEIGHTS: Record<Density, number> = { compact: 32, default: 40, comfortable: 52 };

function readDensity(): Density {
  const v = localStorage.getItem(DENSITY_KEY);
  return v === "compact" || v === "comfortable" ? v : "default";
}

const [density, setDensitySignal] = createSignal<Density>(readDensity());

const PANE_KEY = "pulse.pane";

function readPane(): Pane {
  const v = localStorage.getItem(PANE_KEY) as Pane | null;
  return v && PANES.includes(v) ? v : "right";
}

const [pane, setPaneSignal] = createSignal<Pane>(readPane());

const SPLIT_WIDTH_KEY = "pulse.splitWidth";
const SPLIT_HEIGHT_KEY = "pulse.splitHeight";

function readSplitWidth(): number {
  const v = Number(localStorage.getItem(SPLIT_WIDTH_KEY));
  return v >= 240 && v <= 1200 ? v : 420;
}

function readSplitHeight(): number {
  const v = Number(localStorage.getItem(SPLIT_HEIGHT_KEY));
  return v >= 140 && v <= 800 ? v : 280;
}

const [splitWidth, setSplitWidthSignal] = createSignal<number>(readSplitWidth());
const [splitHeight, setSplitHeightSignal] = createSignal<number>(readSplitHeight());

export const prefs = {
  density,
  rowHeight: () => ROW_HEIGHTS[density()],
  setDensity(d: Density) {
    localStorage.setItem(DENSITY_KEY, d);
    setDensitySignal(d);
  },
  pane,
  setPane(p: Pane) {
    localStorage.setItem(PANE_KEY, p);
    setPaneSignal(p);
  },
  splitWidth,
  setSplitWidth(w: number) {
    localStorage.setItem(SPLIT_WIDTH_KEY, String(w));
    setSplitWidthSignal(w);
  },
  splitHeight,
  setSplitHeight(h: number) {
    localStorage.setItem(SPLIT_HEIGHT_KEY, String(h));
    setSplitHeightSignal(h);
  },
};
