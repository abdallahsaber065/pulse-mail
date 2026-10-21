// Reading pane layout logic (pure; no DOM or storage).

/** Where an opened conversation shows: beside the list, under it, or full page. */
export type Pane = "right" | "bottom" | "full";
export const PANES: Pane[] = ["right", "bottom", "full"];

/**
 * Layout actually used: split modes need room (right ≥ 1024 px wide,
 * bottom ≥ 640 px tall); otherwise the reader takes the full page.
 */
export function effectivePane(p: Pane, width: number, height: number): Pane {
  if (p === "right" && width < 1024) return "full";
  if (p === "bottom" && height < 640) return "full";
  return p;
}

export function nextPane(p: Pane): Pane {
  return PANES[(PANES.indexOf(p) + 1) % PANES.length];
}
