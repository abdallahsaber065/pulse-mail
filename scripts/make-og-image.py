#!/usr/bin/env python3
"""Generate site/assets/og-image.png (1200x630) from the app logo + inbox shot."""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "site" / "assets" / "og-image.png"
LOGO = ROOT / "docs" / "pulse-mail-logo.png"
INBOX = ROOT / "docs" / "assets" / "inbox.png"
FONT_DIR = Path("/usr/share/fonts/truetype/macos")


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(str(FONT_DIR / name), size)


def rounded(im: Image.Image, radius: int) -> Image.Image:
    mask = Image.new("L", im.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, *im.size), radius=radius, fill=255)
    out = im.convert("RGBA")
    out.putalpha(mask)
    return out


def window_frame(shot: Image.Image, bar_h: int = 36) -> Image.Image:
    """Desktop chrome: traffic lights + title bar around a screenshot."""
    pad = 1
    w, h = shot.size
    frame = Image.new("RGBA", (w + pad * 2, h + bar_h + pad), (27, 29, 34, 255))
    bar = Image.new("RGBA", (w, bar_h), (22, 24, 28, 255))
    d = ImageDraw.Draw(bar)
    d.line([(0, bar_h - 1), (w, bar_h - 1)], fill=(44, 47, 53, 255))
    for i, color in enumerate(((255, 95, 87), (254, 188, 46), (40, 200, 64))):
        x = 14 + i * 16
        d.ellipse((x, 12, x + 10, 22), fill=color)
    d.text((w // 2 - 28, 9), "Pulse Mail", font=font("Inter-Medium.ttf", 13), fill=(154, 160, 166))
    frame.paste(bar, (pad, pad))
    frame.paste(shot, (pad, pad + bar_h))
    return rounded(frame, 22)


def main() -> None:
    W, H = 1200, 630
    canvas = Image.new("RGB", (W, H), "#111317")
    overlay = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(overlay)

    d.ellipse((-260, -300, 560, 380), fill=(11, 87, 208, 90))
    d.ellipse((640, -80, 1480, 640), fill=(168, 199, 250, 36))
    d.ellipse((380, 420, 980, 860), fill=(11, 87, 208, 28))
    glow = overlay.filter(ImageFilter.GaussianBlur(72))
    canvas = Image.alpha_composite(canvas.convert("RGBA"), glow)

    draw = ImageDraw.Draw(canvas)
    logo = Image.open(LOGO).convert("RGBA")
    logo.thumbnail((96, 96), Image.Resampling.LANCZOS)
    canvas.paste(logo, (64, 86), logo)

    title = font("Inter-Bold.ttf", 58)
    sub = font("Inter-Medium.ttf", 24)
    small = font("Inter-Regular.ttf", 18)

    draw.text((64, 208), "Pulse Mail", font=title, fill="#f8fafd")
    draw.multiline_text(
        (64, 286),
        "A Gmail client that lives in\nthe tray, not in a browser tab.",
        font=sub,
        fill="#c4c7c5",
        spacing=8,
    )
    draw.text((64, 548), "Open source  ·  Local-first  ·  MIT", font=small, fill="#9aa0a6")

    shot = Image.open(INBOX).convert("RGBA")
    shot = shot.resize((720, int(720 * shot.height / shot.width)), Image.Resampling.LANCZOS)
    framed = window_frame(shot)

    shadow = Image.new("RGBA", (framed.width + 48, framed.height + 48), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    sd.rounded_rectangle((10, 18, framed.width + 10, framed.height + 18), radius=26, fill=(0, 0, 0, 110))
    shadow = shadow.filter(ImageFilter.GaussianBlur(16))
    canvas.paste(shadow, (548, 48), shadow)
    canvas.paste(framed, (568, 52), framed)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    canvas.convert("RGB").save(OUT, "PNG", optimize=True)
    print(f"wrote {OUT} {OUT.stat().st_size} bytes {canvas.size}")


if __name__ == "__main__":
    main()
