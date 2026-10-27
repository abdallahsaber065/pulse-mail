import { createSignal } from "solid-js";

export type ThemePref = "system" | "light" | "dark";
const KEY = "pulse.theme";

function bootPref(): ThemePref {
  const fromUrl = new URLSearchParams(window.location.search).get("theme");
  if (fromUrl === "light" || fromUrl === "dark" || fromUrl === "system") {
    localStorage.setItem(KEY, fromUrl);
    return fromUrl;
  }
  const stored = localStorage.getItem(KEY);
  return stored === "light" || stored === "dark" || stored === "system" ? stored : "system";
}

const media = window.matchMedia("(prefers-color-scheme: dark)");
const [pref, setPrefSignal] = createSignal<ThemePref>(bootPref());
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
