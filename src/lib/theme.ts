import { createSignal } from "solid-js";

export type ThemePref = "system" | "light" | "dark";
const KEY = "pulse.theme";

const media = window.matchMedia("(prefers-color-scheme: dark)");
const [pref, setPrefSignal] = createSignal<ThemePref>((localStorage.getItem(KEY) as ThemePref) || "system");
const [isDark, setIsDark] = createSignal(false);

function apply() {
  const dark = pref() === "dark" || (pref() === "system" && media.matches);
  setIsDark(dark);
  document.documentElement.classList.toggle("dark", dark);
}

media.addEventListener("change", apply);
apply();

export const theme = {
  pref,
  isDark,
  set(p: ThemePref) {
    localStorage.setItem(KEY, p);
    setPrefSignal(p);
    apply();
  },
  cycle() {
    const order: ThemePref[] = ["system", "light", "dark"];
    theme.set(order[(order.indexOf(pref()) + 1) % order.length]);
  },
};
