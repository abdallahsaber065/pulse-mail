#!/usr/bin/env python3
"""Capture full-window demo screenshots (fictional mailbox only)."""

from __future__ import annotations

import time
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "assets"
ART = Path("/opt/cursor/artifacts/pages-polish/app")
BASE = "http://localhost:1422"
W, H = 1440, 900


def shot(page, name: str) -> Path:
    OUT.mkdir(parents=True, exist_ok=True)
    ART.mkdir(parents=True, exist_ok=True)
    dest = OUT / f"{name}.png"
    page.screenshot(path=str(dest), full_page=False, type="png")
    art = ART / f"{name}.png"
    art.write_bytes(dest.read_bytes())
    print(f"  {dest.name} {dest.stat().st_size}")
    return dest


def scroll_settings_to(page, heading: str) -> None:
    page.locator('[aria-label="Settings"] .overflow-y-auto').evaluate(
        """(el, heading) => {
          const h = [...el.querySelectorAll('h3')].find(
            (n) => (n.textContent || '').trim() === heading
          );
          if (!h) return;
          const er = el.getBoundingClientRect();
          const r = h.getBoundingClientRect();
          el.scrollTop += r.top - er.top - 16;
        }""",
        heading,
    )
    page.wait_for_timeout(200)


def open_view(page, query: str, wait_text: str | None = None, wait_sel: str | None = None, delay_ms: int = 400):
    url = f"{BASE}/?shot=1&{query}" if query else f"{BASE}/?shot=1"
    page.goto(url, wait_until="networkidle")
    page.wait_for_selector("text=Pulse")
    if wait_sel:
        page.wait_for_selector(wait_sel, timeout=8000)
    if wait_text:
        page.wait_for_selector(f"text={wait_text}", timeout=8000)
    page.wait_for_timeout(delay_ms)


def main() -> None:
    with sync_playwright() as p:
        browser = p.chromium.launch(channel="chrome", headless=True)
        context = browser.new_context(
            viewport={"width": W, "height": H},
            device_scale_factor=1,
        )
        context.add_init_script(
            """
            localStorage.setItem('pulse.splitWidth', '500');
            localStorage.setItem('pulse.pane', 'right');
            localStorage.setItem('pulse.density', 'default');
            localStorage.removeItem('pm.accountView');
            """
        )
        page = context.new_page()

        print("capturing demo screenshots")
        open_view(page, "theme=light", wait_text="Northwind Auth")
        shot(page, "inbox")
        open_view(page, "theme=dark", wait_text="Northwind Auth")
        shot(page, "inbox-dark")

        open_view(page, "theme=light&thread=t0", wait_text="Your Northwind sign-in code", delay_ms=900)
        shot(page, "thread")
        open_view(page, "theme=dark&thread=t0", wait_text="Your Northwind sign-in code", delay_ms=900)
        shot(page, "thread-dark")

        open_view(page, "theme=light&compose=max", wait_sel='[role="dialog"][aria-label="New message"]', delay_ms=900)
        shot(page, "compose")
        open_view(page, "theme=dark&compose=max", wait_sel='[role="dialog"][aria-label="New message"]', delay_ms=900)
        shot(page, "compose-dark")

        open_view(page, "theme=light&settings=1", wait_text="Appearance", delay_ms=400)
        scroll_settings_to(page, "Storage")
        shot(page, "settings")
        open_view(page, "theme=dark&settings=1", wait_text="Appearance", delay_ms=400)
        scroll_settings_to(page, "Storage")
        shot(page, "settings-dark")

        open_view(page, "theme=light&settings=account", wait_text="This account", delay_ms=500)
        shot(page, "settings-account")

        open_view(page, "theme=light&q=northwind", wait_text="Northwind Auth", delay_ms=1400)
        shot(page, "search")

        open_view(page, "theme=light&label=Clients", wait_text="Mira Okonkwo", delay_ms=500)
        shot(page, "labels")

        open_view(page, "theme=light&toast=1", wait_sel="[data-demo-toast]", delay_ms=900)
        shot(page, "otp")
        open_view(page, "theme=dark&toast=1", wait_sel="[data-demo-toast]", delay_ms=900)
        shot(page, "otp-dark")

        browser.close()


if __name__ == "__main__":
    main()
