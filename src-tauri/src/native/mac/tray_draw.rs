//! Port of `MenuBarIcon.swift` (the stacked-stubs template icon, drawn in code, with the title
//! drawn into the same image) and of the slide in `MenuBarView.swift` (`MenuBarLabel.animate`,
//! `MenuBarSlide.steps`). Drawing uses AppKit's bitmap contexts, which work on any thread.

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep,
    NSColor, NSCompositingOperation, NSDeviceRGBColorSpace, NSFont, NSFontAttributeName,
    NSFontWeightRegular, NSForegroundColorAttributeName, NSGraphicsContext,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSPoint, NSRect, NSSize, NSString};

/// The icon is 18 × 18 pt (MenuBarIcon.image).
pub const ICON_SIDE: f64 = 18.0;
/// Room between the icon and the title.
const GAP: f64 = 4.0;
/// Pixels per point in the rendered image; the status item shows it 18 pt high.
pub const SCALE: f64 = 2.0;

/// A rendered template image: premultiplied RGBA (black, so only alpha matters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub rgba: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

impl Rendered {
    /// Width in points, as the menu bar shows it.
    pub fn width_pt(&self) -> f64 {
        f64::from(self.width_px) / SCALE
    }
}

fn text(title: &str) -> Retained<NSAttributedString> {
    let size = NSFont::menuBarFontOfSize(0.0).pointSize();
    // SAFETY: an immutable CGFloat constant exported by AppKit.
    let weight = unsafe { NSFontWeightRegular };
    let font = NSFont::monospacedDigitSystemFontOfSize_weight(size, weight);
    let black = NSColor::blackColor();
    // SAFETY: immutable NSString constants exported by AppKit.
    let (font_key, color_key) = unsafe { (NSFontAttributeName, NSForegroundColorAttributeName) };
    let objects: [&objc2::runtime::AnyObject; 2] = [&font, &black];
    let attrs: Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> =
        NSDictionary::from_slices(&[font_key, color_key], &objects);
    // SAFETY: the attributes are the documented key/value types (NSFont, NSColor).
    unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(title), &attrs) }
}

/// The room the title takes right of the icon, gap included; 0 with no title.
pub fn text_width(title: &str) -> f64 {
    if title.is_empty() {
        0.0
    } else {
        GAP + text(title).size().width.ceil()
    }
}

/// A stub: a rounded rectangle.
fn stub_rect(rect: NSRect) -> Retained<NSBezierPath> {
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 1.4, 1.4)
}

fn notches(rect: NSRect, radius: f64) -> [Retained<NSBezierPath>; 2] {
    let oval = |x: f64| {
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(x - radius, rect.origin.y + rect.size.height / 2.0 - radius),
            NSSize::new(radius * 2.0, radius * 2.0),
        ))
    };
    [oval(rect.origin.x), oval(rect.origin.x + rect.size.width)]
}

fn inset(r: NSRect, d: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(r.origin.x + d, r.origin.y + d),
        NSSize::new(r.size.width - 2.0 * d, r.size.height - 2.0 * d),
    )
}

fn clear(ctx: &NSGraphicsContext, path: &NSBezierPath) {
    ctx.setCompositingOperation(NSCompositingOperation::Clear);
    path.fill();
    ctx.setCompositingOperation(NSCompositingOperation::SourceOver);
}

/// `MenuBarIcon.draw`, offset by `origin` (the icon's bottom-left in the image).
fn draw_icon(ctx: &NSGraphicsContext, origin: NSPoint) {
    NSColor::blackColor().set();
    let size = NSSize::new(12.0, 7.5);
    let at = |x: f64, y: f64| NSRect::new(NSPoint::new(origin.x + x, origin.y + y), size);
    let frames = [at(5.0, 9.5), at(3.0, 5.75), at(1.0, 2.0)];
    for (i, frame) in frames.iter().enumerate() {
        let back = i < frames.len() - 1;
        let shape = stub_rect(*frame);
        clear(ctx, &stub_rect(inset(*frame, -0.8)));
        if back {
            let outline = stub_rect(inset(*frame, 0.6));
            outline.setLineWidth(1.2);
            outline.stroke();
        } else {
            shape.fill();
        }
        for n in notches(*frame, 1.7) {
            clear(ctx, &n);
        }
        if back {
            // Keep the notch outline on the back stubs.
            for n in notches(*frame, 1.7) {
                NSGraphicsContext::saveGraphicsState_class();
                shape.addClip();
                n.setLineWidth(1.2);
                n.stroke();
                NSGraphicsContext::restoreGraphicsState_class();
            }
        }
    }
}

/// Draws into a fresh `width`×`height` pt bitmap at `scale` pixels per point.
fn draw_rep(
    width: f64,
    height: f64,
    scale: f64,
    draw: impl FnOnce(&NSGraphicsContext),
) -> Option<Retained<NSBitmapImageRep>> {
    let (w, h) = (
        (width * scale).ceil() as isize,
        (height * scale).ceil() as isize,
    );
    // SAFETY: null planes let AppKit allocate the buffer; 8-bit RGBA, non-planar, computed row
    // bytes: a supported combination. The colour space name is an AppKit constant.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w,
            h,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    rep.setSize(NSSize::new(w as f64 / scale, h as f64 / scale));
    let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&ctx));
    draw(&ctx);
    ctx.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    Some(rep)
}

/// Draws into a fresh `width`×`height` pt bitmap at [`SCALE`] and returns its pixels.
fn render(width: f64, height: f64, draw: impl FnOnce(&NSGraphicsContext)) -> Option<Rendered> {
    let rep = draw_rep(width, height, SCALE, draw)?;
    let (w, h) = (rep.pixelsWide(), rep.pixelsHigh());
    let row = rep.bytesPerRow() as usize;
    let data = rep.bitmapData();
    if data.is_null() {
        return None;
    }
    let (wu, hu) = (w as usize, h as usize);
    let mut rgba = Vec::with_capacity(wu * hu * 4);
    for y in 0..hu {
        // SAFETY: the rep owns `row * h` bytes at `data`, alive while `rep` is; each row slice
        // is in bounds (row >= w * 4 for 8-bit RGBA).
        let line = unsafe { std::slice::from_raw_parts(data.add(y * row), wu * 4) };
        rgba.extend_from_slice(line);
    }
    Some(Rendered {
        rgba,
        width_px: w as u32,
        height_px: h as u32,
    })
}

/// The icon alone (MenuBarIcon.image).
pub fn icon() -> Option<Rendered> {
    render(ICON_SIDE, ICON_SIDE, |ctx| {
        draw_icon(ctx, NSPoint::new(0.0, 0.0))
    })
}

/// The icon alone as a PNG at `scale` (1 → 18 px, 2 → 36 px): the committed
/// `icons/tray-icon.png` / `tray-icon@2x.png` are written by the `write_tray_icons` test.
pub fn icon_png(scale: f64) -> Option<Vec<u8>> {
    let rep = draw_rep(ICON_SIDE, ICON_SIDE, scale, |ctx| {
        draw_icon(ctx, NSPoint::new(0.0, 0.0))
    })?;
    let props: Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> = NSDictionary::new();
    // SAFETY: an empty property dictionary is valid for PNG.
    let data =
        unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) }?;
    Some(data.to_vec())
}

/// `MenuBarIcon.label`: the icon, then the title, as one template image. `visible` (0 up to
/// `text_width`) shows only the part of the title nearest the icon, so it can slide.
pub fn label(title: &str, visible: Option<f64>) -> Option<Rendered> {
    let full = text_width(title);
    let shown = visible.unwrap_or(full).clamp(0.0, full);
    if shown <= 0.0 {
        return icon();
    }
    let text = text(title);
    let text_size = text.size();
    let height = ICON_SIDE.max(text_size.height.ceil());
    render(ICON_SIDE + shown, height, |ctx| {
        draw_icon(ctx, NSPoint::new(0.0, (height - ICON_SIDE) / 2.0));
        // The text's start sits against the icon; what doesn't fit yet is past the right edge.
        text.drawAtPoint(NSPoint::new(
            ICON_SIDE + GAP,
            (height - text_size.height) / 2.0,
        ));
    })
}

/// `MenuBarSlide.steps`: the widths the text passes through, eased out, ending on the target.
pub fn steps(from: f64, to: f64, count: usize) -> Vec<f64> {
    (1..=count)
        .map(|i| {
            let t = i as f64 / count as f64;
            let eased = 1.0 - (1.0 - t).powi(3);
            if i == count {
                to
            } else {
                from + (to - from) * eased
            }
        })
        .collect()
}

/// The slide's state, `MenuBarLabel`'s `visible` and `sliding`, plus a generation that stands in
/// for cancelling the Swift `Task`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Slide {
    visible: Option<f64>,
    sliding: Option<String>,
    generation: u64,
}

/// Steps per slide and the pause after each (MB-4, POLL-9).
pub const SLIDE_STEPS: usize = 12;
pub const SLIDE_STEP_MS: u64 = 18;

impl Slide {
    /// `MenuBarLabel.animate(from:to:)`, called when the title changes. Returns the generation and
    /// widths to step through, or `None` when nothing slides (same width, or Reduce Motion).
    pub fn begin(
        &mut self,
        old: &str,
        new: &str,
        width: impl Fn(&str) -> f64,
        reduce_motion: bool,
    ) -> Option<(u64, Vec<f64>)> {
        let from = self.visible.unwrap_or_else(|| width(old));
        let to = width(new);
        if from == to || reduce_motion {
            return None;
        }
        self.generation += 1;
        // Growing shows the new text as it comes out; shrinking keeps the old text going back in.
        self.sliding = if to < from {
            Some(self.sliding.take().unwrap_or_else(|| old.to_owned()))
        } else {
            None
        };
        Some((self.generation, steps(from, to, SLIDE_STEPS)))
    }

    /// One step of slide `generation`; false once a newer slide has replaced it.
    pub fn step(&mut self, generation: u64, width: f64) -> bool {
        if generation != self.generation {
            return false;
        }
        self.visible = Some(width);
        true
    }

    /// The end of slide `generation`: back to drawing the current title in full.
    pub fn finish(&mut self, generation: u64) {
        if generation == self.generation {
            self.visible = None;
            self.sliding = None;
        }
    }

    /// What to draw for `title`: `(sliding ?? title, visible)`.
    pub fn shown<'a>(&'a self, title: &'a str) -> (&'a str, Option<f64>) {
        (self.sliding.as_deref().unwrap_or(title), self.visible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_like_width(r: &Rendered) -> f64 {
        r.width_pt()
    }

    // Port of Tests/ITopWrapTests/MenuBarSlideTests.swift. Widths are in points, rounded up to
    // the 0.5 pt pixel grid of the 2× image.
    fn grid(w: f64) -> f64 {
        (w * SCALE).ceil() / SCALE
    }

    #[test]
    fn icon_only() {
        assert_eq!(text_width(""), 0.0);
        assert_eq!(png_like_width(&label("", None).expect("icon")), ICON_SIDE);
    }

    #[test]
    fn full() {
        let text = text_width("00:12:40");
        assert!(text > 4.0);
        assert_eq!(
            png_like_width(&label("00:12:40", None).expect("label")),
            grid(ICON_SIDE + text)
        );
    }

    #[test]
    fn partial() {
        let text = text_width("00:12:40");
        assert_eq!(
            png_like_width(&label("00:12:40", Some(text / 2.0)).expect("half")),
            grid(ICON_SIDE + text / 2.0)
        );
        assert_eq!(
            png_like_width(&label("00:12:40", Some(0.0)).expect("none")),
            ICON_SIDE
        );
    }

    #[test]
    fn steps_ease_out_and_end_on_target() {
        let s = steps(0.0, 60.0, 12);
        assert_eq!(s.len(), 12);
        assert_eq!(s.last().copied(), Some(60.0));
        assert!(s.windows(2).all(|w| w[0] <= w[1]));
        assert!(
            s[0] > 60.0 / 12.0,
            "eases out: the first step is bigger than an even one"
        );
        assert_eq!(steps(60.0, 0.0, 12).last().copied(), Some(0.0));
    }

    #[test]
    fn digits_are_fixed_width() {
        assert_eq!(text_width("00:00:00  3"), text_width("11:11:11  3"));
    }

    #[test]
    fn icon_has_ink_and_holes() {
        let r = icon().expect("icon");
        assert_eq!((r.width_px, r.height_px), (36, 36));
        let alpha: Vec<u8> = r.rgba.chunks(4).map(|p| p[3]).collect();
        assert!(alpha.contains(&255), "the front stub is filled");
        assert!(
            alpha.iter().filter(|&&a| a == 0).count() > 36 * 36 / 3,
            "mostly transparent"
        );
    }

    /// Regenerates the committed template icons: `cargo test -p itop-wrap -- --ignored write_tray_icons`.
    #[test]
    #[ignore = "writes src-tauri/icons"]
    fn write_tray_icons() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons");
        std::fs::write(dir.join("tray-icon.png"), icon_png(1.0).expect("1x")).expect("write 1x");
        std::fs::write(dir.join("tray-icon@2x.png"), icon_png(2.0).expect("2x")).expect("write 2x");
    }

    fn w(s: &str) -> f64 {
        s.len() as f64 * 10.0
    }

    #[test]
    fn slide_grows_with_the_new_text_and_shrinks_with_the_old() {
        let mut slide = Slide::default();
        let (g, widths) = slide.begin("", "00:00:01", w, false).expect("grows");
        assert_eq!(slide.shown("00:00:01"), ("00:00:01", None));
        for x in &widths {
            assert!(slide.step(g, *x));
        }
        assert_eq!(slide.shown("00:00:01").1, Some(80.0));
        slide.finish(g);
        assert_eq!(slide.shown("00:00:01"), ("00:00:01", None));

        let (g, _) = slide.begin("00:00:09", "", w, false).expect("shrinks");
        assert_eq!(
            slide.shown(""),
            ("00:00:09", None),
            "keeps drawing the old text"
        );
        slide.step(g, 40.0);
        // Interrupted mid-slide by a new title: starts from the visible width, old slide stops.
        let (g2, widths) = slide.begin("", "3", w, false).expect("turns around");
        assert!(!slide.step(g, 0.0));
        assert_eq!(widths.last().copied(), Some(10.0));
        assert!(slide.step(g2, widths[0]));
        assert!(widths[0] < 40.0);
    }

    #[test]
    fn no_slide_for_same_width_or_reduce_motion() {
        let mut slide = Slide::default();
        assert_eq!(slide.begin("00:00:01", "00:00:02", w, false), None);
        assert_eq!(slide.begin("", "00:00:02", w, true), None);
    }
}
