//! The picture of the captcha: five letters on a dark background with circles, as a DXT1 `.dds`
//! file for the `PledgeCrest` packet.
// Java: gameserver/mechanics/captcha/Captcha.java (`generateCaptcha`)
// Java: gameserver/util/DDSConverter.java (`convertToDxt1NoTransparency`, `buildHeaderDxt1`)
//
// The DXT1 conversion is Java's, step by step (the colour distance is measured in the 5/6/5 bit
// components without scaling them back, which makes the choice of the two end colours differ from
// a textbook encoder). The picture itself cannot be Java's: Java draws a TrueType font (Comic
// Sans) through `java.awt`. Here the letters come from a 5x7 pixel font, scaled and turned like
// Java turns each letter (a random angle of at most 0.35 rad), so the client gets a picture of
// the same size, colours and layout that a person reads the same way.

use l2_util::rng::WorldRng;

/// `width` and `height` of the picture.
pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 64;

/// Java `CAPTCHA_TEXT_POSSIBILITIES`.
pub const LETTERS: [char; 20] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'K', 'L', 'M', 'P', 'R', 'S', 'T', 'U', 'W', 'X', 'Y', 'Z',
];

/// The 5x7 shape of each letter of [`LETTERS`], one row per entry, bit 4 is the left column.
const GLYPHS: [[u8; 7]; 20] = [
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // A
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110], // B
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110], // C
    [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110], // D
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111], // E
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000], // F
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111], // G
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // H
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001], // K
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111], // L
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001], // M
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000], // P
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001], // R
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110], // S
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // T
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // U
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010], // W
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001], // X
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100], // Y
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111], // Z
];

/// Pixels per glyph pixel.
const SCALE: f64 = 3.2;
/// Java `circlesToDraw`.
const CIRCLES: usize = 8;
/// Java `rotationRange`, radians.
const ROTATION_RANGE: f64 = 0.7;
/// Java `horizMargin`.
const HORIZ_MARGIN: f64 = 20.0;

/// An RGB picture, `0xRRGGBB` per pixel.
pub type Picture = Vec<u32>;

fn put(picture: &mut Picture, x: i32, y: i32, color: u32) {
    if x >= 0 && y >= 0 && (x as usize) < WIDTH && (y as usize) < HEIGHT {
        picture[y as usize * WIDTH + x as usize] = color;
    }
}

/// `Graphics2D.drawOval(x, y, 2r, 2r)`: the outline of a circle (midpoint algorithm).
fn draw_circle(picture: &mut Picture, cx: i32, cy: i32, r: i32, color: u32) {
    if r <= 0 {
        put(picture, cx, cy, color);
        return;
    }
    let (mut x, mut y, mut err) = (r, 0, 1 - r);
    while x >= y {
        for (dx, dy) in [
            (x, y),
            (y, x),
            (-y, x),
            (-x, y),
            (-x, -y),
            (-y, -x),
            (y, -x),
            (x, -y),
        ] {
            put(picture, cx + dx, cy + dy, color);
        }
        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

/// One letter, turned by `angle`, centred at `(cx, cy)`.
fn draw_letter(picture: &mut Picture, letter: char, cx: f64, cy: f64, angle: f64, color: u32) {
    let Some(index) = LETTERS.iter().position(|l| *l == letter) else {
        return;
    };
    let glyph = &GLYPHS[index];
    let (sin, cos) = angle.sin_cos();
    let half = 18_i32;
    for dy in -half..=half {
        for dx in -half..=half {
            // The destination pixel back into the unturned letter.
            let (fx, fy) = (f64::from(dx), f64::from(dy));
            let sx = (fx * cos + fy * sin) / SCALE + 2.5;
            let sy = (-fx * sin + fy * cos) / SCALE + 3.5;
            if sx < 0.0 || sy < 0.0 || sx >= 5.0 || sy >= 7.0 {
                continue;
            }
            let (col, row) = (sx as usize, sy as usize);
            if glyph[row] & (0b10000 >> col) != 0 {
                put(picture, (cx + fx).round() as i32, (cy + fy).round() as i32, color);
            }
        }
    }
}

/// Java `Captcha.generateCaptcha(char[] text)`.
pub fn generate(text: &[char; 5], rng: &mut WorldRng) -> Picture {
    let text_color = 0x26D51E; // (38, 213, 30)
    let circle_color = 0x496497; // (73, 100, 151)
    let mut picture = vec![0x1E1F1F; WIDTH * HEIGHT]; // (30, 31, 31)

    for _ in 0..CIRCLES {
        let radius = (rng.next_f64() * HEIGHT as f64 / 2.0) as i32;
        let x = (rng.next_f64() * WIDTH as f64 - f64::from(radius)) as i32;
        let y = (rng.next_f64() * HEIGHT as f64 - f64::from(radius)) as i32;
        draw_circle(&mut picture, x + radius, y + radius, radius, circle_color);
    }

    let space_per_char = (-HORIZ_MARGIN * 2.0 + WIDTH as f64) / 4.0;
    for (i, letter) in text.iter().enumerate() {
        let angle = (rng.next_f64() - 0.5) * ROTATION_RANGE;
        let cx = HORIZ_MARGIN + space_per_char * i as f64;
        draw_letter(&mut picture, *letter, cx, HEIGHT as f64 / 2.0, angle, text_color);
    }
    picture
}

/// `DDSConverter.distance`.
fn distance(a: [i32; 3], b: [i32; 3]) -> i32 {
    (b[0] - a[0]).pow(2) + (b[1] - a[1]).pow(2) + (b[2] - a[2]).pow(2)
}

/// `getPixel565`.
fn pixel_565(c: [i32; 3]) -> i32 {
    ((c[0] >> 3) << 11) | ((c[1] >> 2) << 5) | (c[2] >> 3)
}

/// `getColor565`: the components stay in their 5/6/5 bit range.
fn color_565(p: i32) -> [i32; 3] {
    [(p & 63488) >> 11, (p & 2016) >> 5, p & 31]
}

/// `determineExtremeColors`: the two colours farthest apart (the first pair wins a tie).
fn extreme_colors(colors: &[[i32; 3]; 16]) -> [usize; 2] {
    let mut best = i32::MIN;
    let mut out = [0usize; 2];
    for j in 0..15 {
        for k in (j + 1)..16 {
            let d = distance(colors[j], colors[k]);
            if d > best {
                best = d;
                out = [j, k];
            }
        }
    }
    out
}

/// `computeBitMask`.
fn bit_mask(colors: &[[i32; 3]; 16], ends: [usize; 2]) -> u32 {
    let c0 = colors[ends[0]];
    let c1 = colors[ends[1]];
    if c0 == c1 {
        return 0;
    }
    let palette = [
        c0,
        c1,
        [
            (2 * c0[0] + c1[0] + 1) / 3,
            (2 * c0[1] + c1[1] + 1) / 3,
            (2 * c0[2] + c1[2] + 1) / 3,
        ],
        [
            (c0[0] + 2 * c1[0] + 1) / 3,
            (c0[1] + 2 * c1[1] + 1) / 3,
            (c0[2] + 2 * c1[2] + 1) / 3,
        ],
    ];
    let mut mask = 0u32;
    for (i, color) in colors.iter().enumerate() {
        let mut best = 0x7fff_ffff;
        let mut chosen = 0u32;
        for (index, entry) in palette.iter().enumerate() {
            let d = distance(*color, *entry);
            if d < best {
                best = d;
                chosen = index as u32;
            }
        }
        mask |= chosen << (i * 2);
    }
    mask
}

/// `buildHeaderDxt1`: the 128 bytes of a DXT1 `.dds` file.
fn header_dxt1(width: usize, height: usize) -> Vec<u8> {
    let mut h = Vec::with_capacity(128);
    h.extend_from_slice(b"DDS ");
    h.extend_from_slice(&124u32.to_le_bytes());
    h.extend_from_slice(&0xa1007u32.to_le_bytes());
    h.extend_from_slice(&(height as u32).to_le_bytes());
    h.extend_from_slice(&(width as u32).to_le_bytes());
    h.extend_from_slice(&((width * height / 2) as u32).to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&[0u8; 44]);
    h.extend_from_slice(&32u32.to_le_bytes());
    h.extend_from_slice(&4u32.to_le_bytes());
    h.extend_from_slice(b"DXT1");
    for _ in 0..5 {
        h.extend_from_slice(&0u32.to_le_bytes());
    }
    h.extend_from_slice(&4096u32.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&[0u8; 12]);
    h
}

/// `DDSConverter.convertToDxt1NoTransparency` of a picture whose sides are multiples of 4.
pub fn convert_to_dds(picture: &[u32], width: usize, height: usize) -> Vec<u8> {
    let mut out = header_dxt1(width, height);
    for block_y in 0..height / 4 {
        for block_x in 0..width / 4 {
            let mut pixels = [0i32; 16];
            let mut colors = [[0i32; 3]; 16];
            for j in 0..16 {
                let rgb = picture[(block_y * 4 + j / 4) * width + block_x * 4 + j % 4];
                let c888 = [
                    ((rgb >> 16) & 0xff) as i32,
                    ((rgb >> 8) & 0xff) as i32,
                    (rgb & 0xff) as i32,
                ];
                pixels[j] = pixel_565(c888);
                colors[j] = color_565(pixels[j]);
            }
            let mut ends = extreme_colors(&colors);
            if pixels[ends[0]] < pixels[ends[1]] {
                ends.swap(0, 1);
            }
            out.extend_from_slice(&(pixels[ends[0]] as u16).to_le_bytes());
            out.extend_from_slice(&(pixels[ends[1]] as u16).to_le_bytes());
            out.extend_from_slice(&bit_mask(&colors, ends).to_le_bytes());
        }
    }
    out
}

/// The `.dds` bytes of the captcha picture of `text`.
pub fn captcha_dds(text: &[char; 5], rng: &mut WorldRng) -> Vec<u8> {
    convert_to_dds(&generate(text, rng), WIDTH, HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_is_the_one_of_a_dxt1_file() {
        let h = header_dxt1(256, 64);
        assert_eq!(h.len(), 128);
        assert_eq!(&h[0..4], b"DDS ");
        assert_eq!(u32::from_le_bytes([h[4], h[5], h[6], h[7]]), 124);
        assert_eq!(u32::from_le_bytes([h[8], h[9], h[10], h[11]]), 0xa1007);
        assert_eq!(u32::from_le_bytes([h[12], h[13], h[14], h[15]]), 64);
        assert_eq!(u32::from_le_bytes([h[16], h[17], h[18], h[19]]), 256);
        assert_eq!(u32::from_le_bytes([h[20], h[21], h[22], h[23]]), 256 * 64 / 2);
        assert_eq!(&h[84..88], b"DXT1");
        assert_eq!(u32::from_le_bytes([h[108], h[109], h[110], h[111]]), 4096);
    }

    #[test]
    fn a_flat_block_has_two_equal_ends_and_no_bits() {
        // 4x4 of (38, 213, 30): 565 = (4, 53, 3) -> 0x26d5 packs as (4 << 11) | (53 << 5) | 3.
        let picture = vec![0x26D51E; 16];
        let dds = convert_to_dds(&picture, 4, 4);
        assert_eq!(dds.len(), 128 + 8);
        let color = ((38 >> 3) << 11) | ((213 >> 2) << 5) | (30 >> 3);
        assert_eq!(&dds[128..130], &(color as u16).to_le_bytes());
        assert_eq!(&dds[130..132], &(color as u16).to_le_bytes());
        assert_eq!(&dds[132..136], &[0, 0, 0, 0]);
    }

    #[test]
    fn two_colours_pick_the_ends_and_the_bits() {
        // Left half black, right half white: the ends are white (bigger 565 value) and black,
        // the black pixels use index 1 and the white ones index 0.
        let mut picture = vec![0u32; 16];
        for y in 0..4 {
            for x in 2..4 {
                picture[y * 4 + x] = 0xFFFFFF;
            }
        }
        let dds = convert_to_dds(&picture, 4, 4);
        assert_eq!(&dds[128..130], &0xFFFFu16.to_le_bytes());
        assert_eq!(&dds[130..132], &0u16.to_le_bytes());
        let mask = u32::from_le_bytes([dds[132], dds[133], dds[134], dds[135]]);
        for i in 0..16 {
            let expected = if i % 4 < 2 { 1 } else { 0 };
            assert_eq!((mask >> (i * 2)) & 3, expected, "pixel {i}");
        }
    }

    #[test]
    fn the_picture_has_letters_and_the_file_has_the_right_size() {
        let mut rng = WorldRng::from_seed(7);
        let picture = generate(&['A', 'B', 'C', 'D', 'E'], &mut rng);
        assert_eq!(picture.len(), WIDTH * HEIGHT);
        let green = picture.iter().filter(|p| **p == 0x26D51E).count();
        assert!(green > 300, "{green}");
        let dds = captcha_dds(&['Z', 'Y', 'X', 'W', 'U'], &mut rng);
        assert_eq!(dds.len(), 128 + WIDTH * HEIGHT / 2);
    }

    #[test]
    fn every_letter_has_a_glyph() {
        for (i, glyph) in GLYPHS.iter().enumerate() {
            assert!(glyph.iter().any(|row| *row != 0), "{}", LETTERS[i]);
        }
    }
}
