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


def main() -> None:
    W, H = 1200, 630
    canvas = Image.new("RGB", (W, H), "#0b1018")
    overlay = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(overlay)

    # Soft blue/violet glows matching the neon envelope logo.
    d.ellipse((-220, -280, 620, 420), fill=(11, 87, 208, 70))
    d.ellipse((720, 80, 1500, 820), fill=(90, 40, 180, 50))
    d.ellipse((480, -120, 980, 280), fill=(0, 180, 220, 28))
    glow = overlay.filter(ImageFilter.GaussianBlur(80))
    canvas = Image.alpha_composite(canvas.convert("RGBA"), glow)

    # Hairline grid, very quiet.
    grid = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    g = ImageDraw.Draw(grid)
    for x in range(0, W, 40):
        g.line([(x, 0), (x, H)], fill=(255, 255, 255, 10))
    for y in range(0, H, 40):
        g.line([(0, y), (W, y)], fill=(255, 255, 255, 10))
    canvas = Image.alpha_composite(canvas, grid)

    draw = ImageDraw.Draw(canvas)
    logo = Image.open(LOGO).convert("RGBA")
    logo.thumbnail((112, 112), Image.Resampling.LANCZOS)
    canvas.paste(logo, (72, 78), logo)

    title = font("Inter-Bold.ttf", 64)
    sub = font("Inter-Medium.ttf", 26)
    small = font("Inter-Regular.ttf", 20)

    draw.text((72, 214), "Pulse Mail", font=title, fill="#f8fafd")
    draw.multiline_text(
        (72, 300),
        "A Gmail client that lives in\nthe tray, not in a browser tab.",
        font=sub,
        fill="#c4c7c5",
        spacing=8,
    )
    draw.text((72, 540), "Open source  ·  Local-first  ·  MIT", font=small, fill="#9aa0a6")

    shot = Image.open(INBOX).convert("RGBA")
    shot = shot.resize((720, int(720 * shot.height / shot.width)), Image.Resampling.LANCZOS)
    shot = rounded(shot, 22)
    # Drop shadow
    shadow = Image.new("RGBA", (shot.width + 40, shot.height + 40), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    sd.rounded_rectangle((8, 16, shot.width + 8, shot.height + 16), radius=24, fill=(0, 0, 0, 90))
    shadow = shadow.filter(ImageFilter.GaussianBlur(18))
    canvas.paste(shadow, (560, 70), shadow)
    canvas.paste(shot, (580, 70), shot)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    canvas.convert("RGB").save(OUT, "PNG", optimize=True)
    print(f"wrote {OUT} {OUT.stat().st_size} bytes {canvas.size}")


if __name__ == "__main__":
    main()
