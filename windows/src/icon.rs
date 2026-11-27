// Profile and app icons, drawn the same way as the macOS app, then packed into .ico files.
// Coordinates are on a 1024 canvas (like the macOS code) and scaled to the size asked for.
use crate::core::Look;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::sync::OnceLock;
use tiny_skia::*;

pub fn hex(s: &str) -> Option<[u8; 3]> {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).ok().filter(|_| s.trim_start_matches('#').len() == 6)?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}
pub fn to_hex(c: [u8; 3]) -> String { format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]) }

fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let k = r * 0.4477; // r - r*0.5523: cubic control offset for a quarter circle
    let mut p = PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(x + w - r, y);
    p.cubic_to(x + w - k, y, x + w, y + k, x + w, y + r);
    p.line_to(x + w, y + h - r);
    p.cubic_to(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h);
    p.line_to(x + r, y + h);
    p.cubic_to(x + k, y + h, x, y + h - k, x, y + h - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + k, x + k, y, x + r, y);
    p.close();
    p.finish().unwrap()
}

fn solid(c: [u8; 3], a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], a);
    p.anti_alias = true;
    p
}

fn gradient(from: [u8; 3], to: [u8; 3], a: Point, b: Point) -> Paint<'static> {
    let stops = vec![GradientStop::new(0.0, Color::from_rgba8(from[0], from[1], from[2], 255)), GradientStop::new(1.0, Color::from_rgba8(to[0], to[1], to[2], 255))];
    Paint { shader: LinearGradient::new(a, b, stops, SpreadMode::Pad, Transform::identity()).unwrap(), anti_alias: true, ..Default::default() }
}

/// Draw `img` scaled into the rect, optionally aspect-filling and clipping to `clip`.
fn draw_image(dst: &mut Pixmap, img: &Pixmap, x: f32, y: f32, w: f32, h: f32, fill: bool, clip: Option<&Path>) {
    let (sx, sy) = (w / img.width() as f32, h / img.height() as f32);
    let s = if fill { sx.max(sy) } else { sx.min(sy) };
    let (iw, ih) = (img.width() as f32 * s, img.height() as f32 * s);
    let t = Transform::from_row(s, 0.0, 0.0, s, x + (w - iw) / 2.0, y + (h - ih) / 2.0);
    let mask = clip.map(|p| { let mut m = Mask::new(dst.width(), dst.height()).unwrap(); m.fill_path(p, FillRule::Winding, true, Transform::identity()); m });
    dst.draw_pixmap(0, 0, img.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, t, mask.as_ref());
}

// MARK: Text

/// Bold UI font first, then emoji. Emoji come out as white silhouettes (color fonts need DirectWrite).
fn fonts() -> &'static [FontVec] {
    static F: OnceLock<Vec<FontVec>> = OnceLock::new();
    F.get_or_init(|| {
        let paths: &[&str] = if cfg!(windows) {
            &[r"C:\Windows\Fonts\segoeuib.ttf", r"C:\Windows\Fonts\seguiemj.ttf", r"C:\Windows\Fonts\seguisym.ttf", r"C:\Windows\Fonts\arialbd.ttf"]
        } else {
            &["/System/Library/Fonts/Supplemental/Arial Bold.ttf", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"]
        };
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        paths.iter().filter_map(|p| std::fs::read(p.replace(r"C:\Windows", &windir)).ok()).filter_map(|d| FontVec::try_from_vec(d).ok()).collect()
    })
}

/// Draw white `text` with its em size `em`, centered on (cx, cy).
fn draw_text(dst: &mut Pixmap, text: &str, em: f32, cx: f32, cy: f32) {
    let fonts = fonts();
    let mut glyphs = vec![];
    let mut x = 0.0;
    for c in text.chars().filter(|c| !matches!(*c as u32, 0xFE0E | 0xFE0F | 0x200D | 0x1F3FB..=0x1F3FF)) {
        let Some(f) = fonts.iter().find(|f| f.glyph_id(c).0 != 0) else { continue };
        let scaled = f.as_scaled(PxScale::from(em * f.height_unscaled() / f.units_per_em().unwrap_or(1000.0)));
        let mut g = scaled.scaled_glyph(c);
        g.position = ab_glyph::point(x, 0.0);
        x += scaled.h_advance(g.id);
        glyphs.push((f, g));
    }
    let outlines: Vec<_> = glyphs.into_iter().filter_map(|(f, g)| f.outline_glyph(g)).collect();
    let Some(b) = outlines.iter().map(|o| o.px_bounds()).reduce(|a, b| ab_glyph::Rect { min: ab_glyph::point(a.min.x.min(b.min.x), a.min.y.min(b.min.y)), max: ab_glyph::point(a.max.x.max(b.max.x), a.max.y.max(b.max.y)) }) else { return };
    let (ox, oy) = (cx - (b.min.x + b.max.x) / 2.0, cy - (b.min.y + b.max.y) / 2.0);
    let (w, h) = (dst.width() as i32, dst.height() as i32);
    let px = dst.pixels_mut();
    for o in &outlines {
        let ob = o.px_bounds();
        o.draw(|gx, gy, cov| {
            let (x, y) = ((ob.min.x + ox).round() as i32 + gx as i32, (ob.min.y + oy).round() as i32 + gy as i32);
            if x < 0 || y < 0 || x >= w || y >= h { return }
            let d = &mut px[(y * w + x) as usize];
            let a = cov.clamp(0.0, 1.0);
            let mix = |dc: u8| (255.0 * a + dc as f32 * (1.0 - a)).round() as u8; // premultiplied white over dst
            *d = PremultipliedColorU8::from_rgba(mix(d.red()), mix(d.green()), mix(d.blue()), mix(d.alpha())).unwrap();
        });
    }
}

// MARK: Images

pub fn load_image(path: &str) -> Option<Pixmap> {
    let img = image::open(path).ok()?.to_rgba8();
    from_rgba(img.width(), img.height(), img.into_raw(), false)
}

/// RGBA bytes (straight or premultiplied alpha) to a pixmap.
pub fn from_rgba(w: u32, h: u32, mut data: Vec<u8>, premultiplied: bool) -> Option<Pixmap> {
    for p in data.chunks_exact_mut(4) {
        if !premultiplied { for i in 0..3 { p[i] = (p[i] as u16 * p[3] as u16 / 255) as u8 } }
        for i in 0..3 { p[i] = p[i].min(p[3]) }
    }
    Pixmap::from_vec(data, IntSize::from_wh(w, h)?)
}

// MARK: Icons

/// A profile icon: app icon (or custom image) with an optional badge, or a whole-icon emoji/letter tile.
pub fn render_icon(base: Option<&Pixmap>, look: &Look, size: u32) -> Pixmap {
    let mut pm = Pixmap::new(size, size).unwrap();
    let s = size as f32 / 1024.0;
    let tile = (64.0 * s, 896.0 * s, 180.0 * s); // inset, side, corner radius: Windows icons fill more of the canvas than macOS ones
    if look.framed {
        let inner = render_icon(base, &Look { framed: false, ..look.clone() }, size);
        let frame = rrect(tile.0, tile.0, tile.1, tile.1, tile.2);
        pm.fill_path(&frame, &gradient([77, 77, 77], [41, 41, 41], Point::from_xy(0.0, tile.0), Point::from_xy(0.0, tile.0 + tile.1)), FillRule::Winding, Transform::identity(), None);
        let i = 150.0 * s;
        draw_image(&mut pm, &inner, i, i, size as f32 - 2.0 * i, size as f32 - 2.0 * i, false, None);
        return pm;
    }
    let color = hex(&look.color).unwrap_or([10, 132, 255]);
    let tile_path = rrect(tile.0, tile.0, tile.1, tile.1, tile.2);
    if look.glyph {
        pm.fill_path(&tile_path, &solid(color, 255), FillRule::Winding, Transform::identity(), None);
        let text = if look.badge.is_empty() { "?" } else { &look.badge };
        draw_text(&mut pm, text, if crate::core::graphemes(text) > 1 { 380.0 * s } else { 560.0 * s }, 512.0 * s, 512.0 * s);
        return pm;
    }
    match (look.image_path.as_deref().and_then(load_image), base) {
        (Some(img), _) => draw_image(&mut pm, &img, tile.0, tile.0, tile.1, tile.1, true, Some(&tile_path)),
        (None, Some(b)) => draw_image(&mut pm, b, 0.0, 0.0, size as f32, size as f32, false, None),
        (None, None) => { pm.fill_path(&tile_path, &solid([60, 60, 67], 255), FillRule::Winding, Transform::identity(), None); }
    }
    if !look.badge.is_empty() {
        // Bigger than on macOS: taskbar icons are 24–32 px, not Dock-sized.
        let (bx, bd) = (580.0 * s, 430.0 * s);
        let c = bx + bd / 2.0;
        let circle = PathBuilder::from_circle(c, c, bd / 2.0).unwrap();
        pm.fill_path(&circle, &solid(color, 255), FillRule::Winding, Transform::identity(), None);
        let ring = PathBuilder::from_circle(c, c, bd / 2.0 - 10.0 * s).unwrap();
        pm.stroke_path(&ring, &solid([255, 255, 255], 255), &Stroke { width: 20.0 * s, ..Default::default() }, Transform::identity(), None);
        draw_text(&mut pm, &look.badge, if crate::core::graphemes(&look.badge) > 1 { 190.0 * s } else { 260.0 * s }, c, c);
    }
    pm
}

/// AIGENT's own icon: a violet tile holding a 2×2 grid of app tiles, one lit up.
pub fn render_app_icon(size: u32) -> Pixmap {
    let mut pm = Pixmap::new(size, size).unwrap();
    let s = size as f32 / 1024.0;
    let (t, side) = (64.0 * s, 896.0 * s);
    let a = Point::from_xy(t, t);
    let b = Point::from_xy(t + side * 0.5, t + side * 0.866); // -60° in the macOS code (y-up)
    pm.fill_path(&rrect(t, t, side, side, 180.0 * s), &gradient([0x5B, 0x4B, 0xF5], [0xB3, 0x47, 0xE8], a, b), FillRule::Winding, Transform::identity(), None);
    let (pad, gap) = (130.0 * s, 48.0 * s);
    let cell = (side - 2.0 * pad - gap) / 2.0;
    for (i, (cx, cy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
        let (x, y) = (t + pad + cx as f32 * (cell + gap), t + pad + cy as f32 * (cell + gap));
        let paint = if i == 1 { solid([0xFF, 0x9F, 0x6B], 255) } else { solid([255, 255, 255], 235) };
        pm.fill_path(&rrect(x, y, cell, cell, 60.0 * s), &paint, FillRule::Winding, Transform::identity(), None);
    }
    pm
}

/// .ico with PNG entries (Vista+): ICONDIR, one 16-byte entry per size, then the PNGs.
pub fn ico(render: impl Fn(u32) -> Pixmap) -> Vec<u8> {
    let pngs: Vec<(u32, Vec<u8>)> = [256, 64, 48, 32, 24, 16].iter().map(|&s| (s, render(s).encode_png().unwrap())).collect();
    let mut out = vec![0, 0, 1, 0, pngs.len() as u8, 0];
    let mut offset = 6 + 16 * pngs.len() as u32;
    for (s, png) in &pngs {
        let d = if *s >= 256 { 0 } else { *s as u8 };
        out.extend([d, d, 0, 0, 1, 0, 32, 0]);
        out.extend((png.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in pngs { out.extend(png) }
    out
}

pub fn write_ico(path: &std::path::Path, render: impl Fn(u32) -> Pixmap) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, ico(render)).map_err(|e| format!("Couldn't save the icon: {e}"))
}

pub fn self_test() {
    assert_eq!(to_hex(hex("#BF5AF2").unwrap()), "#BF5AF2", "hex round-trip");
    assert!(hex("#12345").is_none() && hex("nope").is_none(), "bad hex");
    let data = ico(render_app_icon);
    assert_eq!(&data[..6], &[0, 0, 1, 0, 6, 0], "ico header");
    let off = u32::from_le_bytes(data[18..22].try_into().unwrap()) as usize;
    assert_eq!(&data[off..off + 8], b"\x89PNG\r\n\x1a\n", "first entry is a PNG");
    let img = image::load_from_memory(&data).expect("ico decodes").to_rgba8();
    assert_eq!(img.width(), 256, "largest entry first");
    // Badge is drawn bottom-right in the look's color; the glyph tile is filled with it.
    let look = Look { badge: "W".into(), color: "#FF0000".into(), glyph: true, ..Default::default() };
    let px = render_icon(None, &look, 64).pixel(8, 32).unwrap();
    assert!(px.red() == 255 && px.green() == 0 && px.alpha() == 255, "glyph tile color");
    let badge = render_icon(None, &Look { glyph: false, ..look.clone() }, 64).pixel(49, 39).unwrap();
    assert!(badge.red() > 200 && badge.green() < 60, "badge color");
    if !fonts().is_empty() {
        let tile = render_icon(None, &look, 256);
        assert!((100..156).any(|x| tile.pixel(x, 128).unwrap().green() > 200), "letter drawn in white");
    }
}
