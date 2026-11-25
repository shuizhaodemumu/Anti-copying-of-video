use gtk4::{gdk, glib, prelude::*};
use image::{imageops, GrayImage, ImageFormat, RgbImage};

const PAGE: &[u8] = include_bytes!("../assets/text.jpg");
const MOVIE: &[u8] = include_bytes!("../assets/video.mjpg");
const LEVELS: usize = 16;
// Maximum per-phase change in linear luminance before 8-bit rounding.
// Neighboring tiles use opposite signs, avoiding full-field bright/dark frames.
const LUMA_SHIFT: f64 = 0.028;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strength {
    Standard,
    Strong,
    Latent,
    Balanced,
}

pub struct Frames {
    pub normal: gdk::Texture,
    pub a: gdk::Texture,
    pub b: gdk::Texture,
}

pub struct ExperimentMedia {
    source_page: RgbImage,
    page: Frames,
    strength: Strength,
    strong_palette: StrongPalette,
    movie: Vec<&'static [u8]>,
    current: Option<(usize, Frames)>,
    palette: Palette,
}

impl ExperimentMedia {
    pub fn new() -> Result<Self, String> {
        let palette = Palette::new();
        let strong_palette = StrongPalette::new();
        let source_page = decode(PAGE)?;
        let strength = Strength::Standard;
        let page = prepare(
            source_page.clone(),
            &PAGE_GLYPHS,
            strength,
            &palette,
            &strong_palette,
        );
        let movie = split_movie(MOVIE)?;
        Ok(Self {
            source_page,
            page,
            strength,
            strong_palette,
            movie,
            current: None,
            palette,
        })
    }

    pub fn set_strength(&mut self, strength: Strength) {
        if self.strength != strength {
            self.page = prepare(
                self.source_page.clone(),
                &PAGE_GLYPHS,
                strength,
                &self.palette,
                &self.strong_palette,
            );
            self.current = None;
            self.strength = strength;
        }
    }

    pub fn frame_count(&self) -> usize {
        self.movie.len()
    }

    pub fn text(&self) -> &Frames {
        &self.page
    }

    pub fn video(&mut self, index: usize) -> Result<&Frames, String> {
        let data = self.movie.get(index).ok_or("视频帧号越界")?;
        if self.current.as_ref().map(|(i, _)| *i) != Some(index) {
            self.current = Some((
                index,
                prepare(
                    decode(data)?,
                    GLYPH_MASK,
                    self.strength,
                    &self.palette,
                    &self.strong_palette,
                ),
            ));
        }
        Ok(&self.current.as_ref().unwrap().1)
    }
}

// The bundled encoder writes independent JPEGs without metadata or padding.
// JPEG entropy byte stuffing prevents this EOI+SOI boundary inside scan data.
fn split_movie(data: &[u8]) -> Result<Vec<&[u8]>, String> {
    if !data.starts_with(&[0xff, 0xd8]) || !data.ends_with(&[0xff, 0xd9]) {
        return Err("内置视频缺少 JPEG 帧边界".into());
    }
    let mut frames = Vec::new();
    let mut start = 0;
    for (offset, marker) in data.windows(4).enumerate() {
        if marker == [0xff, 0xd9, 0xff, 0xd8] {
            let end = offset + 2;
            frames.push(&data[start..end]);
            start = end;
        }
    }
    frames.push(&data[start..]);
    Ok(frames)
}

fn srgb_to_linear(x: u8) -> f64 {
    let x = f64::from(x) / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(x: f64) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let encoded = if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

struct Channel {
    max_level: usize,
    pair: [[u8; 2]; LEVELS + 1],
}

// Each axis combines a Rec.709-isoluminant color offset with a small
// luminance offset. Both phases use one gamut-safe scale in linear-light sRGB.
struct Palette([[[Channel; 256]; 3]; 2]);

impl Palette {
    fn new() -> Self {
        let axes = [(0.088, -0.035), (-0.050, 0.138)];
        Self(axes.map(|(r, b)| Self::axis(r, b, LUMA_SHIFT)))
    }

    fn axis(r: f64, b: f64, luma: f64) -> [[Channel; 256]; 3] {
        let chroma = [r, -(0.2126 * r + 0.0722 * b) / 0.7152, b];
        let offsets = chroma.map(|delta| delta + luma);
        std::array::from_fn(|channel| {
            let shift = offsets[channel];
            std::array::from_fn(|index| {
                let x = srgb_to_linear(index as u8);
                let headroom = x.min(1.0 - x);
                let max_level = ((headroom / shift.abs()).min(1.0) * LEVELS as f64) as usize;
                let pair = std::array::from_fn(|level| {
                    let delta = shift * level as f64 / LEVELS as f64;
                    [linear_to_srgb(x + delta), linear_to_srgb(x - delta)]
                });
                Channel { max_level, pair }
            })
        })
    }

    #[inline]
    fn pair(axis: &[[Channel; 256]; 3], rgb: &[u8], phase: usize) -> ([u8; 3], [u8; 3]) {
        let [r, g, b] = std::array::from_fn(|c| &axis[c][rgb[c] as usize]);
        let level = r.max_level.min(g.max_level).min(b.max_level);
        let [r, g, b] = [r.pair[level], g.pair[level], b.pair[level]];
        (
            [r[phase], g[phase], b[phase]],
            [r[1 - phase], g[1 - phase], b[1 - phase]],
        )
    }

    fn apply(&self, image: &RgbImage) -> (Vec<u8>, Vec<u8>) {
        const TILE: usize = 12;
        let width = image.width() as usize;
        let pixels = image.as_raw();
        let mut a = Vec::with_capacity(pixels.len());
        let mut b = Vec::with_capacity(pixels.len());
        for (y, row) in pixels.chunks_exact(width * 3).enumerate() {
            let ty = y / TILE;
            for (tx, tile) in row.chunks(TILE * 3).enumerate() {
                // Neighboring tiles alternate phase; the second chroma axis
                // prevents the entire field from becoming a single color cast.
                let phase = (tx + ty) % 2;
                let axis = &self.0[(tx / 2 + ty) % 2];
                for rgb in tile.chunks_exact(3) {
                    let (positive, negative) = Self::pair(axis, rgb, phase);
                    a.extend_from_slice(&positive);
                    b.extend_from_slice(&negative);
                }
            }
        }
        (a, b)
    }
}

// Strong uses local contrast to reserve a little gamut headroom around strokes.
// Preserve the old A/B texture outside high-contrast neighborhoods, including large
// uniform backgrounds. The signed additions stay complementary in linear light.
const EDGE_LEVELS: usize = 4;
const LOCAL_COMPRESSION: f64 = 0.088;
const STRONG_LUMA: f64 = 0.038;
const EDGE_TILE: usize = 4;

struct StrongPalette {
    levels: Vec<Palette>,
}

impl StrongPalette {
    fn new() -> Self {
        let mut levels = Vec::with_capacity(EDGE_LEVELS);
        for level in 1..=EDGE_LEVELS {
            let weight = level as f64 / EDGE_LEVELS as f64;
            let luma = STRONG_LUMA * weight;
            let palette = Palette(std::array::from_fn(|axis| {
                let (r, b): (f64, f64) = [(0.134, -0.068), (-0.080, 0.175)][axis];
                let chroma = [r, -(0.2126 * r + 0.0722 * b) / 0.7152, b];
                std::array::from_fn(|c| {
                    let offset = chroma[c] * weight;
                    std::array::from_fn(|value| {
                        let x = srgb_to_linear(value as u8);
                        let center = x + (0.5 - x) * LOCAL_COMPRESSION * weight;
                        // Reserve the neutral luminance shift before allocating extra
                        // chroma. Near black ink, this keeps modulation after clipping.
                        let available = center.min(1.0 - center) - luma;
                        let max_level =
                            ((available / offset.abs()).clamp(0.0, 1.0) * LEVELS as f64) as usize;
                        let pair = std::array::from_fn(|i| {
                            let shift = luma + offset * i as f64 / LEVELS as f64;
                            [
                                linear_to_srgb(center + shift),
                                linear_to_srgb(center - shift),
                            ]
                        });
                        Channel { max_level, pair }
                    })
                })
            }));
            levels.push(palette);
        }
        Self { levels }
    }

    fn apply(&self, image: &RgbImage, standard: &Palette) -> (Vec<u8>, Vec<u8>) {
        let (mut a, mut b) = standard.apply(image);
        let (width, height) = image.dimensions();
        let (width, height) = (width as usize, height as usize);
        let cols = width.div_ceil(EDGE_TILE);
        let rows = height.div_ceil(EDGE_TILE);
        let mut extrema = vec![(255u8, 0u8); rows * cols];
        for (y, row) in image.as_raw().chunks_exact(width * 3).enumerate() {
            let tile_row = &mut extrema[(y / EDGE_TILE) * cols..][..cols];
            for (range, pixels) in tile_row.iter_mut().zip(row.chunks(EDGE_TILE * 3)) {
                for rgb in pixels.chunks_exact(3) {
                    // Encoded luminance is only used for edge selection. All rendering
                    // and A/B averages use the linear sRGB transfer function.
                    let y = ((54 * rgb[0] as u32 + 183 * rgb[1] as u32 + 19 * rgb[2] as u32) >> 8)
                        as u8;
                    range.0 = range.0.min(y);
                    range.1 = range.1.max(y);
                }
            }
        }
        for ty in 0..rows {
            for tx in 0..cols {
                let (mut low, mut high) = (255, 0);
                for y in ty.saturating_sub(1)..=(ty + 1).min(rows - 1) {
                    for &(a, b) in &extrema
                        [y * cols + tx.saturating_sub(1)..=y * cols + (tx + 1).min(cols - 1)]
                    {
                        low = low.min(a);
                        high = high.max(b);
                    }
                }
                let weight = ((high - low).saturating_sub(32) as usize / 24).min(EDGE_LEVELS);
                if weight == 0 {
                    continue;
                }
                let palette = &self.levels[weight - 1];
                for y in ty * EDGE_TILE..((ty + 1) * EDGE_TILE).min(height) {
                    for x in tx * EDGE_TILE..((tx + 1) * EDGE_TILE).min(width) {
                        let at = (y * width + x) * 3;
                        let pixel = &image.as_raw()[at..at + 3];
                        // Do not add a stronger alternating pattern to saturated reds.
                        if pixel[0] > pixel[1].saturating_add(42)
                            && pixel[0] > pixel[2].saturating_add(42)
                        {
                            a[at..at + 3].copy_from_slice(pixel);
                            b[at..at + 3].copy_from_slice(pixel);
                            continue;
                        }
                        // Opposite signs within each six-pixel horizontal pair keep
                        // the field balanced. Break up rows to avoid a coarse grid.
                        let row = y / 3;
                        let cell = (x + row % 3) / 3;
                        let pair = cell / 2;
                        let hash = (pair as u32).wrapping_mul(0x9e3779b9)
                            ^ (row as u32).wrapping_mul(0x85ebca6b);
                        let phase = (cell ^ ((hash >> 23) as usize)) & 1;
                        let axis = ((hash >> 18) as usize) & 1;
                        let (first, second) = Palette::pair(&palette.0[axis], pixel, phase);
                        a[at..at + 3].copy_from_slice(&first);
                        b[at..at + 3].copy_from_slice(&second);
                    }
                }
            }
        }
        (a, b)
    }
}

// The mask only controls the *direction and amplitude* of a pixel's A/B difference.
// Its target mean comes from the actual, quantized 0.2 frame pair. No permanent text
// is composited over the source or over the mean. Panel response and exposure may vary.
const MASK_WIDTH: usize = 800;
const MASK_HEIGHT: usize = 450;
const GLYPH_MASK: &[u8; MASK_WIDTH * MASK_HEIGHT] = include_bytes!("../assets/latent-mask.raw");
// Reuse the original antialiased wordmark and scale it once. Each copy fits
// three quarters of a third of the picture width, with equal cell margins.
static PAGE_GLYPHS: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    let width = MASK_WIDTH as u32;
    let height = MASK_HEIGHT as u32;
    let source = GrayImage::from_raw(width, height, GLYPH_MASK.to_vec()).unwrap();
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (x, y, p) in source.enumerate_pixels() {
        if p[0] != 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    let mut grid = GrayImage::new(width, height);
    if right > left && bottom > top {
        let ink = imageops::crop_imm(&source, left, top, right - left, bottom - top).to_image();
        let word_w = width / 4;
        let word_h = (ink.height() * word_w + ink.width() / 2) / ink.width();
        let word = imageops::resize(&ink, word_w, word_h, imageops::FilterType::Triangle);
        for row in 0..3 {
            for col in 0..3 {
                let x = (2 * col + 1) * width / 6 - word_w / 2;
                let y = (2 * row + 1) * height / 6 - word_h / 2;
                imageops::replace(&mut grid, &word, x.into(), y.into());
            }
        }
    }
    grid.into_raw()
});

static GLYPH_INK: std::sync::LazyLock<GlyphInk> = std::sync::LazyLock::new(GlyphInk::new);

struct GlyphInk {
    linear: [i32; 256],
    encoded: Vec<u8>,
}

impl GlyphInk {
    fn new() -> Self {
        Self {
            linear: std::array::from_fn(|i| (srgb_to_linear(i as u8) * 65535.0).round() as i32),
            encoded: (0..=65535)
                .map(|v| linear_to_srgb(f64::from(v) / 65535.0))
                .collect(),
        }
    }

    // Jointly quantize a channel pair. Given a desired phase A, choose the adjacent
    // 8-bit pair whose linear-light sum is closest to the original A+B sum.
    fn encode_pair(&self, sum: i32, wanted_a: i32) -> (u8, u8) {
        let closest = self.encoded[wanted_a.clamp(0, 65535) as usize];
        let mut best = (u8::MIN, u8::MIN);
        let mut error = i32::MAX;
        for a in [
            closest,
            closest.saturating_sub(1),
            closest.saturating_add(1),
        ] {
            let wanted_b = sum - self.linear[a as usize];
            let b = self.encoded[wanted_b.clamp(0, 65535) as usize];
            let residual = (self.linear[b as usize] - wanted_b).abs();
            if residual < error {
                best = (a, b);
                error = residual;
            }
        }
        best
    }

    fn recolor(&self, a: &mut [u8], b: &mut [u8], coverage: u8) {
        // Signed full A-B color difference in 16-bit linear sRGB. Phase A is cool
        // and phase B warm. Rec.709 luminance differs by only ~0.027 at full gain;
        // much of the short-exposure signal is in the red/blue opposition.
        const DIFFERENCE: [i32; 3] = [-22937, 6160, 31457];
        let before_a = [a[0], a[1], a[2]].map(|v| self.linear[v as usize]);
        let before_b = [b[0], b[1], b[2]].map(|v| self.linear[v as usize]);
        let sum: [i32; 3] = std::array::from_fn(|c| before_a[c] + before_b[c]);
        // One common gain prevents separate channel clipping from stamping the
        // glyph into the mean, including over light paper or nearly black strokes.
        const FULL: i32 = 4096;
        let mut gain = FULL;
        for c in 0..3 {
            let room = sum[c].min(2 * 65535 - sum[c]);
            gain = gain.min(room * FULL / DIFFERENCE[c].abs());
        }
        for c in 0..3 {
            let glyph_difference = DIFFERENCE[c] * gain / FULL;
            let old_difference = before_a[c] - before_b[c];
            let blend = i32::from(coverage);
            let difference = ((255 - blend) * old_difference + blend * glyph_difference) / 255;
            (a[c], b[c]) = self.encode_pair(sum[c], (sum[c] + difference) / 2);
        }
    }

    fn apply(&self, image: &RgbImage, mask: &[u8], a: &mut [u8], b: &mut [u8]) {
        if image.dimensions() != (MASK_WIDTH as u32, MASK_HEIGHT as u32) {
            return;
        }
        for (position, &coverage) in mask.iter().enumerate() {
            if coverage == 0 {
                continue;
            }
            let i = position * 3;
            let [r, g, blue] = [
                image.as_raw()[i],
                image.as_raw()[i + 1],
                image.as_raw()[i + 2],
            ];
            if r > g.saturating_add(42) && r > blue.saturating_add(42) {
                continue;
            }
            self.recolor(&mut a[i..i + 3], &mut b[i..i + 3], coverage);
        }
    }
}

// One gamut-safe, equal-luminance color pair per source RGB, without the
// luminance offset used by the legacy profiles. The mask can only swap its
// two quantized endpoints, so it cannot stamp lettering into their mean.
static BALANCED_AXIS: std::sync::LazyLock<[[Channel; 256]; 3]> =
    std::sync::LazyLock::new(|| Palette::axis(-0.086, 0.12, 0.0));

fn balanced_frames(image: &RgbImage, glyph: &[u8]) -> (Vec<u8>, Vec<u8>) {
    // A fixed Bayer threshold covers the antialiased edge of the lettering.
    // Only the phase order changes, including at these fractional-ink pixels.
    const COVERAGE: [[u8; 4]; 4] = [
        [8, 136, 40, 168],
        [200, 72, 232, 104],
        [56, 184, 24, 152],
        [248, 120, 216, 88],
    ];
    let width = image.width() as usize;
    let registered = image.dimensions() == (MASK_WIDTH as u32, MASK_HEIGHT as u32)
        && glyph.len() == MASK_WIDTH * MASK_HEIGHT;
    let axis = &*BALANCED_AXIS;
    let mut a = Vec::with_capacity(image.as_raw().len());
    let mut b = Vec::with_capacity(image.as_raw().len());
    for (i, pixel) in image.as_raw().chunks_exact(3).enumerate() {
        let x = i % width;
        let y = i / width;
        // Do not add chromatic modulation to saturated red content.
        if pixel[0] > pixel[1].saturating_add(42) && pixel[0] > pixel[2].saturating_add(42) {
            a.extend_from_slice(pixel);
            b.extend_from_slice(pixel);
            continue;
        }
        let row = y / 3;
        let checker = ((x + row % 4) / 4 + row) & 1;
        let ink = registered && glyph[i] > COVERAGE[y % 4][x % 4];
        let phase = if ink { 0 } else { checker };
        let (first, second) = Palette::pair(axis, pixel, phase);
        a.extend_from_slice(&first);
        b.extend_from_slice(&second);
    }
    (a, b)
}

fn decode(data: &[u8]) -> Result<RgbImage, String> {
    image::load_from_memory_with_format(data, ImageFormat::Jpeg)
        .map(|image| image.into_rgb8())
        .map_err(|e| format!("内置画面解码失败：{e}"))
}

fn texture(w: i32, h: i32, data: Vec<u8>) -> gdk::Texture {
    let storage = glib::Bytes::from_owned(data);
    gdk::MemoryTexture::new(w, h, gdk::MemoryFormat::R8g8b8, &storage, w as usize * 3).upcast()
}

fn prepare(
    image: RgbImage,
    mask: &[u8],
    strength: Strength,
    palette: &Palette,
    strong: &StrongPalette,
) -> Frames {
    let (w, h) = (image.width() as i32, image.height() as i32);
    let (a, b) = match strength {
        Strength::Standard => palette.apply(&image),
        Strength::Strong => strong.apply(&image, palette),
        Strength::Latent => {
            let (mut a, mut b) = strong.apply(&image, palette);
            GLYPH_INK.apply(&image, mask, &mut a, &mut b);
            (a, b)
        }
        Strength::Balanced => balanced_frames(&image, mask),
    };
    Frames {
        normal: texture(w, h, image.into_raw()),
        a: texture(w, h, a),
        b: texture(w, h, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_and_color_phases_recombine_without_whole_field_luminance_swings() {
        let palette = Palette::new();
        let luma = |rgb: [f64; 3]| 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        for axis in &palette.0 {
            for gray in 0u8..=255 {
                for rgb in [[gray; 3], [gray, gray.wrapping_add(77), 255 - gray]] {
                    let (a, b) = Palette::pair(axis, &rgb, 0);
                    let [a, b, center] = [a, b, rgb].map(|pixel| pixel.map(srgb_to_linear));
                    for c in 0..3 {
                        // A static row exposed for 1/30 s at 60 Hz integrates one
                        // whole A/B pair. Allow only the 8-bit endpoint rounding.
                        assert!(((a[c] + b[c]) / 2.0 - center[c]).abs() <= 0.0046);
                    }
                    for phase in [a, b] {
                        assert!((luma(phase) - luma(center)).abs() < LUMA_SHIFT + 0.0046);
                    }
                    assert!((luma(a) - luma(b)).abs() < 0.08);
                }
            }
            let (a, b) = Palette::pair(axis, &[128; 3], 0);
            assert!(a.iter().zip(b).any(|(x, y)| x.abs_diff(y) > 20));
            let phase_difference = luma(a.map(srgb_to_linear)) - luma(b.map(srgb_to_linear));
            assert!(phase_difference > 0.04);
            assert_eq!((b, a), Palette::pair(axis, &[128; 3], 1));
            assert_eq!(([0; 3], [0; 3]), Palette::pair(axis, &[0; 3], 0));
            assert_eq!(([255; 3], [255; 3]), Palette::pair(axis, &[255; 3], 0));
        }
    }

    fn luma(rgb: &[u8]) -> f64 {
        0.2126 * srgb_to_linear(rgb[0])
            + 0.7152 * srgb_to_linear(rgb[1])
            + 0.0722 * srgb_to_linear(rgb[2])
    }

    fn pair_mean(a: &[u8], b: &[u8], c: usize) -> f64 {
        (srgb_to_linear(a[c]) + srgb_to_linear(b[c])) * 0.5
    }

    fn cool_chroma(rgb: &[u8]) -> f64 {
        srgb_to_linear(rgb[2]) - srgb_to_linear(rgb[0])
    }

    #[test]
    fn page_has_nine_complete_identical_marks_centered_in_separate_cells() {
        let source =
            GrayImage::from_raw(MASK_WIDTH as u32, MASK_HEIGHT as u32, PAGE_GLYPHS.clone())
                .unwrap();
        let copy = imageops::crop_imm(&source, 33, 54, 200, 42).to_image();
        let ink = copy.as_raw().iter().filter(|&&v| v > 0).count();
        assert!(ink > 2_000);
        assert_eq!(PAGE_GLYPHS.iter().filter(|&&v| v > 0).count(), ink * 9);
        for y in [54, 204, 354] {
            for x in [33, 300, 566] {
                assert_eq!(copy, imageops::crop_imm(&source, x, y, 200, 42).to_image());
            }
        }
    }

    #[test]
    fn balanced_glyph_only_reorders_colors_and_preserves_single_phase_chroma() {
        let movie = split_movie(MOVIE).unwrap();
        for (data, mask) in [
            (PAGE, PAGE_GLYPHS.as_slice()),
            (movie[0], GLYPH_MASK),
            (movie[149], GLYPH_MASK),
            (movie[299], GLYPH_MASK),
        ] {
            let image = decode(data).unwrap();
            let (reference_a, reference_b) = balanced_frames(&image, &[]);
            let (a, b) = balanced_frames(&image, mask);
            let mut coherent = 0;
            let mut swaps = 0;
            for (i, &coverage) in mask.iter().enumerate() {
                let index = 3 * i..3 * i + 3;
                let first = &a[index.clone()];
                let second = &b[index.clone()];
                let reference = (&reference_a[index.clone()], &reference_b[index.clone()]);
                if (first, second) != reference {
                    assert!(coverage > 0);
                    assert_eq!((second, first), reference);
                    swaps += 1;
                }
                assert!((luma(first) - luma(second)).abs() < 0.009);
                if coverage == 255 && cool_chroma(first) - cool_chroma(second) > 0.15 {
                    coherent += 1;
                }
                for c in 0..3 {
                    let src = srgb_to_linear(image.as_raw()[index.start + c]);
                    assert!((pair_mean(first, second, c) - src).abs() < 0.0046);
                }
            }
            assert!(
                swaps > 900 && coherent > 1500,
                "{swaps} swaps; {coherent} coherent pixels"
            );
        }
    }

    #[test]
    fn balanced_skips_red_and_extreme_neutrals() {
        let pixels = vec![255, 32, 32, 0, 0, 0, 255, 255, 255];
        let image = RgbImage::from_raw(3, 1, pixels.clone()).unwrap();
        assert_eq!(
            balanced_frames(&image, GLYPH_MASK),
            (pixels.clone(), pixels)
        );
    }

    #[test]
    fn glyph_inverts_its_chroma_in_each_half_without_a_persistent_imprint() {
        let ink = &*GLYPH_INK;
        // Old tile pairs may have either orientation. Full mask coverage gives
        // them a common orientation, independently of old checker alignment.
        for (base_a, base_b) in [
            ([35; 3], [35; 3]),
            ([128; 3], [128; 3]),
            ([239; 3], [239; 3]),
            ([185, 210, 225], [218, 171, 158]),
            ([218, 171, 158], [185, 210, 225]),
        ] {
            let (mut a, mut b) = (base_a, base_b);
            ink.recolor(&mut a, &mut b, 255);
            let old_center = (cool_chroma(&base_a) + cool_chroma(&base_b)) * 0.5;
            assert!(cool_chroma(&a) - old_center > 0.02);
            assert!(cool_chroma(&b) - old_center < -0.02);
            assert!((luma(&a) - luma(&b)).abs() < 0.045);
            for coverage in [1, 64, 128, 192, 255] {
                let (mut a, mut b) = (base_a, base_b);
                ink.recolor(&mut a, &mut b, coverage);
                for c in 0..3 {
                    assert!((pair_mean(&a, &b, c) - pair_mean(&base_a, &base_b, c)).abs() < 0.0045);
                }
            }
        }
        for gray in [0, 255] {
            let (mut a, mut b) = ([gray; 3], [gray; 3]);
            ink.recolor(&mut a, &mut b, 255);
            assert_eq!((a, b), ([gray; 3], [gray; 3]));
        }
    }

    #[test]
    fn shipped_scenes_keep_the_old_field_around_camera_visible_glyph_strokes() {
        let standard = Palette::new();
        let strong = StrongPalette::new();
        let movie = split_movie(MOVIE).unwrap();
        for (data, mask) in [
            (PAGE, PAGE_GLYPHS.as_slice()),
            (movie[0], GLYPH_MASK),
            (movie[149], GLYPH_MASK),
            (movie[299], GLYPH_MASK),
        ] {
            let scene = decode(data).unwrap();
            let (old_a, old_b) = strong.apply(&scene, &standard);
            let (mut new_a, mut new_b) = (old_a.clone(), old_b.clone());
            GLYPH_INK.apply(&scene, mask, &mut new_a, &mut new_b);
            let mut coherent_strokes = 0;
            for (i, &coverage) in mask.iter().enumerate() {
                let idx = i * 3..i * 3 + 3;
                let (a, b) = (&new_a[idx.clone()], &new_b[idx.clone()]);
                let (base_a, base_b) = (&old_a[idx.clone()], &old_b[idx]);
                if coverage == 0 {
                    assert_eq!((a, b), (base_a, base_b));
                    continue;
                }
                for c in 0..3 {
                    assert!((pair_mean(a, b, c) - pair_mean(base_a, base_b, c)).abs() < 0.0045);
                }
                if coverage == 255 && cool_chroma(a) - cool_chroma(b) > 0.06 {
                    coherent_strokes += 1;
                }
            }
            assert!(
                coherent_strokes > 1500,
                "{coherent_strokes} coherent glyph pixels"
            );
        }
    }

    #[test]
    fn strong_adds_stroke_scale_luma_interference_and_preserves_uniform_areas() {
        // A narrow black stroke on the left and a white stroke on dark gray on the right.
        let scene = RgbImage::from_fn(64, 24, |x, _| {
            let gray = match x {
                14..=16 => 35,
                0..=31 | 46..=48 => 239,
                _ => 45,
            };
            image::Rgb([gray; 3])
        });
        let standard = Palette::new();
        let (old_a, old_b) = standard.apply(&scene);
        let (new_a, new_b) = StrongPalette::new().apply(&scene, &standard);
        for x in [15, 47] {
            let i = (12 * 64 + x) * 3;
            let a = luma(&new_a[i..]);
            let b = luma(&new_b[i..]);
            let old_diff = (luma(&old_a[i..]) - luma(&old_b[i..])).abs();
            assert!((a - b).abs() > old_diff + 0.012);
            assert!((a - b).abs() <= 0.09);
            let source = luma(&scene.as_raw()[i..]);
            let expected = source + (0.5 - source) * LOCAL_COMPRESSION;
            assert!(((a + b) / 2.0 - expected).abs() <= 0.005);
            assert!((expected - source).abs() < 0.045);
        }
        for x in [1, 63] {
            let i = (12 * 64 + x) * 3;
            assert_eq!(&old_a[i..i + 3], &new_a[i..i + 3]);
            assert_eq!(&old_b[i..i + 3], &new_b[i..i + 3]);
        }
    }

    #[test]
    fn every_strong_level_preserves_intended_mean_and_limits_phase_luminance() {
        for (i, palette) in StrongPalette::new().levels.iter().enumerate() {
            let compression = LOCAL_COMPRESSION * (i + 1) as f64 / EDGE_LEVELS as f64;
            for axis in &palette.0 {
                for r in (0..=255u8).step_by(5) {
                    for g in (0..=255u8).step_by(5) {
                        for b in (0..=255u8).step_by(5) {
                            let rgb = [r, g, b];
                            let (a, b) = Palette::pair(axis, &rgb, 0);
                            for c in 0..3 {
                                let source = srgb_to_linear(rgb[c]);
                                let target = source + (0.5 - source) * compression;
                                let mean = (srgb_to_linear(a[c]) + srgb_to_linear(b[c])) * 0.5;
                                assert!((target - mean).abs() < 0.0046);
                            }
                            assert!((luma(&a) - luma(&b)).abs() < 0.09);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn built_in_video_has_300_decodable_independent_frames() {
        let movie = split_movie(MOVIE).unwrap();
        assert_eq!(movie.len(), 300);
        assert_eq!(decode(PAGE).unwrap().dimensions(), (800, 450));
        for frame in movie {
            assert_eq!(decode(frame).unwrap().dimensions(), (800, 450));
        }
        assert!(split_movie(&[0xff, 0xd8]).is_err());
    }

    #[test]
    #[ignore = "run in release mode to measure frame preparation cost"]
    fn measure_decoding_and_composition() {
        use std::time::Instant;
        let palette = Palette::new();
        let strong = StrongPalette::new();
        let movie = split_movie(MOVIE).unwrap();
        for strength in [Strength::Strong, Strength::Latent, Strength::Balanced] {
            let mut elapsed = Vec::new();
            for data in &movie {
                let start = Instant::now();
                let image = decode(data).unwrap();
                if strength == Strength::Balanced {
                    std::hint::black_box(balanced_frames(&image, GLYPH_MASK));
                } else {
                    let (mut a, mut b) = strong.apply(&image, &palette);
                    if strength == Strength::Latent {
                        GLYPH_INK.apply(&image, GLYPH_MASK, &mut a, &mut b);
                    }
                    std::hint::black_box((a, b));
                }
                elapsed.push(start.elapsed());
            }
            elapsed.sort();
            eprintln!(
                "{strength:?} CPU frame preparation: median {:?}, p95 {:?}, max {:?}",
                elapsed[elapsed.len() / 2],
                elapsed[elapsed.len() * 95 / 100],
                elapsed.last().unwrap()
            );
        }
    }
}
