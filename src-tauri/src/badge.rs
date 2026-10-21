//! Unread badge: taskbar overlay (count in a red circle), tray icon dot and
//! tray tooltip. Pure pixel code so it's testable without a window.

/// 3×5 bitmap digits, one row per u8 (bits 2..0 = left..right).
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];
const PLUS: [u8; 5] = [0b000, 0b010, 0b111, 0b010, 0b000];

const RED: [u8; 4] = [0xd9, 0x30, 0x25, 0xff];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

/// Text shown in the badge: "1".."9", "10".."99", "99+" shown as "9+".
pub fn label(count: i64) -> String {
    match count {
        ..=0 => String::new(),
        1..=99 => count.to_string(),
        _ => "9+".into(),
    }
}

fn put(buf: &mut [u8], size: u32, x: i32, y: i32, c: [u8; 4]) {
    if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 {
        return;
    }
    let i = ((y as u32 * size + x as u32) * 4) as usize;
    buf[i..i + 4].copy_from_slice(&c);
}

fn disc(buf: &mut [u8], size: u32, cx: f32, cy: f32, r: f32, c: [u8; 4]) {
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dx * dx + dy * dy <= r * r {
                put(buf, size, x, y, c);
            }
        }
    }
}

/// 32×32 RGBA overlay: red circle with the count in white.
pub fn overlay(count: i64) -> Option<(Vec<u8>, u32, u32)> {
    let text = label(count);
    if text.is_empty() {
        return None;
    }
    const S: u32 = 32;
    let mut buf = vec![0u8; (S * S * 4) as usize];
    disc(&mut buf, S, 16.0, 16.0, 16.0, RED);
    let glyphs: Vec<[u8; 5]> = text
        .chars()
        .map(|ch| ch.to_digit(10).map(|d| DIGITS[d as usize]).unwrap_or(PLUS))
        .collect();
    let scale = if glyphs.len() == 1 { 4 } else { 3 };
    let (gw, gap) = (3 * scale, scale);
    let width = glyphs.len() as i32 * gw + (glyphs.len() as i32 - 1) * gap;
    let (x0, y0) = ((S as i32 - width) / 2, (S as i32 - 5 * scale) / 2);
    for (n, g) in glyphs.iter().enumerate() {
        let gx = x0 + n as i32 * (gw + gap);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            put(&mut buf, S, gx + col * scale + sx, y0 + row as i32 * scale + sy, WHITE);
                        }
                    }
                }
            }
        }
    }
    Some((buf, S, S))
}

/// The tray icon with a red dot (white ring) in the top-right corner.
pub fn with_dot(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = rgba.to_vec();
    if w != h || out.len() != (w * h * 4) as usize {
        return out;
    }
    let r = w as f32 * 0.24;
    let (cx, cy) = (w as f32 - r - 0.5, r + 0.5);
    disc(&mut out, w, cx, cy, r + w as f32 * 0.06, WHITE);
    disc(&mut out, w, cx, cy, r, RED);
    out
}

pub fn tooltip(total: i64, per_account: &[(String, i64)]) -> String {
    let mut t = match total {
        0 => "Pulse Mail · no unread mail".to_string(),
        1 => "Pulse Mail · 1 unread".to_string(),
        n => format!("Pulse Mail · {n} unread"),
    };
    if per_account.len() > 1 {
        for (email, n) in per_account {
            t.push_str(&format!("\n{email}: {n}"));
        }
    }
    // Windows truncates tray tooltips at 127 characters.
    t.chars().take(127).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_overlay() {
        assert_eq!(label(0), "");
        assert_eq!(label(7), "7");
        assert_eq!(label(42), "42");
        assert_eq!(label(250), "9+");
        assert!(overlay(0).is_none());
        let (buf, w, h) = overlay(5).unwrap();
        assert_eq!((w, h, buf.len()), (32, 32, 32 * 32 * 4));
        let px = |x: u32, y: u32| &buf[((y * 32 + x) * 4) as usize..][..4];
        assert_eq!(px(0, 0)[3], 0, "corner stays transparent");
        assert_eq!(px(3, 16), &RED[..], "circle is red");
        assert!(buf.chunks(4).any(|p| p == WHITE), "digit drawn in white");
    }

    #[test]
    fn dot_and_tooltip() {
        let icon = vec![0u8; 32 * 32 * 4];
        let out = with_dot(&icon, 32, 32);
        assert_eq!(&out[((5 * 32 + 26) * 4) as usize..][..4], &RED[..]);
        assert_eq!(out[((31 * 32) * 4 + 3) as usize], 0, "bottom-left untouched");
        assert_eq!(tooltip(1, &[]), "Pulse Mail · 1 unread");
        let t = tooltip(3, &[("a@x.com".into(), 2), ("b@y.com".into(), 1)]);
        assert!(t.contains("a@x.com: 2") && t.contains("b@y.com: 1"));
    }
}
