//! Image intake conversion (LOG-18), port of `ImageIntake.convert` (W/ComposerTextView.swift:31-53)
//! on ImageIO instead of `NSBitmapImageRep`: decode anything macOS can read (HEIC included), scale
//! down to at most 2560 px on the long side (never up, rounded like Swift), and encode PNG or JPEG
//! at quality 0.85. Thread-safe; no AppKit.

use std::ffi::c_void;

use objc2_core_foundation::{
    CFBoolean, CFData, CFDictionary, CFMutableData, CFNumber, CFRetained, CFString, CFType,
    CGPoint, CGRect, CGSize,
};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImage,
    CGImageAlphaInfo, CGInterpolationQuality,
};
use objc2_image_io::{
    CGImageDestination, CGImageSource, kCGImageDestinationLossyCompressionQuality,
    kCGImagePropertyOrientation, kCGImagePropertyPixelHeight, kCGImagePropertyPixelWidth,
    kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform,
    kCGImageSourceThumbnailMaxPixelSize,
};

use crate::native::{ConvertTo, ConvertedImage};

/// `ImageIntake.maxConvertedSide`.
pub const MAX_CONVERTED_SIDE: f64 = 2560.0;

/// The size an image of `w`×`h` pixels is converted to: unchanged up to 2560 on the long side,
/// else scaled so the long side is 2560, each side rounded (Swift's `.rounded()`, half away from 0).
pub fn target_size(w: usize, h: usize) -> (usize, usize) {
    let longest = w.max(h) as f64;
    if longest <= MAX_CONVERTED_SIDE {
        return (w, h);
    }
    let scale = MAX_CONVERTED_SIDE / longest;
    (
        ((w as f64) * scale).round() as usize,
        ((h as f64) * scale).round() as usize,
    )
}

pub fn convert(data: &[u8], to: ConvertTo) -> Option<ConvertedImage> {
    let cf_data = CFData::from_bytes(data);
    // SAFETY: a valid CFData and no options.
    let source = unsafe { CGImageSource::with_data(&cf_data, None) }?;
    // SAFETY: `source` is valid; count is a plain getter.
    if unsafe { source.count() } == 0 {
        return None;
    }
    let orientation = orientation(&source);
    // The stored size from the header, so a huge image is never decoded at full size.
    let image = match header_size(&source) {
        Some((w, h)) => {
            if w == 0 || h == 0 || w.saturating_mul(h) > MAX_SOURCE_PIXELS {
                return None;
            }
            let (tw, th) = target_size(w, h);
            if (tw, th) == (w, h) {
                // SAFETY: index 0 exists (count > 0); no options.
                unsafe { source.image_at_index(0, None) }?
            } else {
                let thumb = thumbnail(&source, tw.max(th))?;
                let size = (CGImage::width(Some(&thumb)), CGImage::height(Some(&thumb)));
                // ImageIO may round the short side differently from Swift: then scale exactly.
                if size == (tw, th) {
                    thumb
                } else {
                    scaled(&thumb, tw, th)?
                }
            }
        }
        None => {
            // SAFETY: index 0 exists (count > 0); no options.
            let image = unsafe { source.image_at_index(0, None) }?;
            let (w, h) = (CGImage::width(Some(&image)), CGImage::height(Some(&image)));
            if w == 0 || h == 0 || w.saturating_mul(h) > MAX_SOURCE_PIXELS {
                return None;
            }
            let (tw, th) = target_size(w, h);
            if (tw, th) == (w, h) {
                image
            } else {
                scaled(&image, tw, th)?
            }
        }
    };
    let (uti, ext) = match to {
        ConvertTo::Png => ("public.png", "png"),
        ConvertTo::Jpeg => ("public.jpeg", "jpg"),
    };
    let out = CFMutableData::new(None, 0)?;
    let uti = CFString::from_static_str(uti);
    // SAFETY: valid mutable data and type string; one image; no options.
    let dest = unsafe { CGImageDestination::with_data(&out, &uti, 1, None) }?;
    let mut keys: Vec<&CFString> = Vec::new();
    let mut values: Vec<CFRetained<CFNumber>> = Vec::new();
    if to == ConvertTo::Jpeg {
        // SAFETY: an immutable CFString constant exported by ImageIO.
        keys.push(unsafe { kCGImageDestinationLossyCompressionQuality });
        values.push(CFNumber::new_f64(0.85));
    }
    if let Some(o) = orientation {
        // SAFETY: as above.
        keys.push(unsafe { kCGImagePropertyOrientation });
        values.push(CFNumber::new_i32(o));
    }
    let value_refs: Vec<&CFType> = values.iter().map(|v| v.as_ref()).collect();
    let props = CFDictionary::<CFString, CFType>::from_slices(&keys, &value_refs);
    // SAFETY: valid destination, image and a CFString → CFNumber dictionary as ImageIO expects.
    let ok = unsafe {
        dest.add_image(&image, Some(props.as_opaque()));
        dest.finalize()
    };
    if !ok {
        return None;
    }
    let bytes = out.to_vec();
    (!bytes.is_empty()).then_some(ConvertedImage { data: bytes, ext })
}

/// Images above this many pixels (stored size) aren't converted: a few bytes of PNG can declare a
/// picture that would take gigabytes to decode. 300 MP is well past any camera or screenshot.
pub const MAX_SOURCE_PIXELS: usize = 300_000_000;

/// The first image's stored pixel size from its header, without decoding it.
fn header_size(source: &CGImageSource) -> Option<(usize, usize)> {
    // SAFETY: index 0 exists (checked by the caller); no options.
    let props = unsafe { source.properties_at_index(0, None) }?;
    let read = |key: &CFString| -> Option<usize> {
        // SAFETY: `props` has CFString keys; the value, if any, is owned by `props`.
        let raw = unsafe { props.value((key as *const CFString).cast::<c_void>()) };
        if raw.is_null() {
            return None;
        }
        // SAFETY: non-null CF object owned by `props`, which outlives this borrow.
        let value: &CFType = unsafe { &*raw.cast::<CFType>() };
        usize::try_from(value.downcast_ref::<CFNumber>()?.as_i64()?).ok()
    };
    // SAFETY: immutable CFString constants exported by ImageIO.
    let (wk, hk) = unsafe { (kCGImagePropertyPixelWidth, kCGImagePropertyPixelHeight) };
    Some((read(wk)?, read(hk)?))
}

/// The first image decoded straight at `max_side` on its long side (ImageIO subsamples), stored
/// orientation kept (the orientation property is written back by the caller, as before).
fn thumbnail(source: &CGImageSource, max_side: usize) -> Option<CFRetained<CGImage>> {
    // SAFETY: immutable CFString constants exported by ImageIO.
    let keys: [&CFString; 3] = unsafe {
        [
            kCGImageSourceCreateThumbnailFromImageAlways,
            kCGImageSourceThumbnailMaxPixelSize,
            kCGImageSourceCreateThumbnailWithTransform,
        ]
    };
    let side = CFNumber::new_i64(i64::try_from(max_side).ok()?);
    let values: [&CFType; 3] = [
        CFBoolean::new(true).as_ref(),
        side.as_ref(),
        CFBoolean::new(false).as_ref(),
    ];
    let options = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    // SAFETY: index 0 exists; a CFString → CFType dictionary of thumbnail options.
    unsafe { source.thumbnail_at_index(0, Some(options.as_opaque())) }
}

/// The EXIF/TIFF orientation (1…8) of the first image, kept so a portrait photo still shows upright.
fn orientation(source: &CGImageSource) -> Option<i32> {
    // SAFETY: index 0 exists (checked by the caller); no options.
    let props = unsafe { source.properties_at_index(0, None) }?;
    // SAFETY: an immutable CFString constant exported by ImageIO.
    let key: &CFString = unsafe { kCGImagePropertyOrientation };
    // SAFETY: `props` is a valid dictionary with CFString keys; the value, if any, is a CF object
    // owned by the dictionary, which outlives this borrow.
    let raw = unsafe { props.value((key as *const CFString).cast::<c_void>()) };
    if raw.is_null() {
        return None;
    }
    // SAFETY: non-null CF object owned by `props` (see above).
    let value: &CFType = unsafe { &*raw.cast::<CFType>() };
    value
        .downcast_ref::<CFNumber>()?
        .as_i32()
        .filter(|o| (2..=8).contains(o))
}

/// Draws `image` into a `w`×`h` RGBA bitmap, like Swift's scaled `NSBitmapImageRep`.
fn scaled(image: &CGImage, w: usize, h: usize) -> Option<CFRetained<CGImage>> {
    let space = CGColorSpace::new_device_rgb()?;
    // SAFETY: a null data pointer lets Core Graphics allocate the buffer; 8 bits per component,
    // bytes-per-row 0 = computed, premultiplied RGBA is a supported bitmap format.
    let ctx: CFRetained<CGContext> = unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            w,
            h,
            8,
            0,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }?;
    CGContext::set_interpolation_quality(Some(&ctx), CGInterpolationQuality::High);
    let rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w as f64, h as f64));
    CGContext::draw_image(Some(&ctx), rect, Some(image));
    CGBitmapContextCreateImage(Some(&ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_scale_down_never_up() {
        assert_eq!(target_size(800, 600), (800, 600));
        assert_eq!(target_size(2560, 100), (2560, 100));
        assert_eq!(target_size(5120, 2880), (2560, 1440));
        assert_eq!(target_size(3000, 4000), (1920, 2560));
        assert_eq!(target_size(4033, 3025), (2560, 1920)); // 1920.19… rounds to 1920
    }

    /// Makes a real image of `w`×`h` with `sips`-free ImageIO (encode a bitmap as `uti`).
    fn make(w: usize, h: usize, uti: &'static str) -> Vec<u8> {
        let space = CGColorSpace::new_device_rgb().expect("rgb");
        // SAFETY: as in `scaled`.
        let ctx = unsafe {
            CGBitmapContextCreate(
                std::ptr::null_mut(),
                w,
                h,
                8,
                0,
                Some(&space),
                CGImageAlphaInfo::PremultipliedLast.0,
            )
        }
        .expect("ctx");
        let img = CGBitmapContextCreateImage(Some(&ctx)).expect("img");
        let out = CFMutableData::new(None, 0).expect("data");
        let uti = CFString::from_static_str(uti);
        // SAFETY: as in `convert`.
        unsafe {
            let dest = CGImageDestination::with_data(&out, &uti, 1, None).expect("dest");
            dest.add_image(&img, None);
            assert!(dest.finalize());
        }
        out.to_vec()
    }

    fn size_of(data: &[u8]) -> (usize, usize) {
        let d = CFData::from_bytes(data);
        // SAFETY: as in `convert`.
        let img = unsafe {
            CGImageSource::with_data(&d, None)
                .expect("src")
                .image_at_index(0, None)
                .expect("img")
        };
        (CGImage::width(Some(&img)), CGImage::height(Some(&img)))
    }

    #[test]
    fn tiff_becomes_png_and_big_bmp_becomes_small_jpeg() {
        let tiff = make(40, 30, "public.tiff");
        let png = convert(&tiff, ConvertTo::Png).expect("png");
        assert_eq!(png.ext, "png");
        assert_eq!(&png.data[..4], b"\x89PNG");
        assert_eq!(size_of(&png.data), (40, 30));

        let bmp = make(3000, 1500, "com.microsoft.bmp");
        let jpg = convert(&bmp, ConvertTo::Jpeg).expect("jpg");
        assert_eq!(jpg.ext, "jpg");
        assert_eq!(&jpg.data[..3], b"\xFF\xD8\xFF");
        assert_eq!(size_of(&jpg.data), (2560, 1280));
    }

    #[test]
    fn heic_converts_when_the_mac_can_encode_it() {
        // Every Apple-silicon Mac can encode HEIC; skip quietly where ImageIO can't.
        let out = CFMutableData::new(None, 0).expect("data");
        let uti = CFString::from_static_str("public.heic");
        // SAFETY: as in `convert`.
        if unsafe { CGImageDestination::with_data(&out, &uti, 1, None) }.is_none() {
            return;
        }
        let heic = make(64, 48, "public.heic");
        let jpg = convert(&heic, ConvertTo::Jpeg).expect("jpg from heic");
        assert_eq!(size_of(&jpg.data), (64, 48));
    }

    #[test]
    fn an_image_too_big_to_decode_is_refused_from_its_header() {
        // A 20000×20000 PNG header (400 MP) with no pixel data: refused before any decoding.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(20_000u32.to_be_bytes());
        png.extend(20_000u32.to_be_bytes());
        png.extend([8, 6, 0, 0, 0]);
        png.extend([0, 0, 0, 0]); // CRC, not checked by the header read
        assert!(convert(&png, ConvertTo::Png).is_none());
    }

    #[test]
    fn not_an_image() {
        assert_eq!(convert(b"hello, this is text", ConvertTo::Jpeg), None);
        assert_eq!(convert(&[], ConvertTo::Png), None);
    }
}
