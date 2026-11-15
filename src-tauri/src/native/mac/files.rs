//! Reading images off the general pasteboard (LOG-14) and writing saved attachments (DET-34) the
//! way the Swift app does, through AppKit and Foundation.

use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{ClassType, msg_send};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString, NSPasteboardTypeTIFF,
    NSPasteboardURLReadingFileURLsOnlyKey,
};
use objc2_foundation::{
    NSArray, NSData, NSDataWritingOptions, NSDictionary, NSNumber, NSString, NSURL,
};

use crate::native::PastedImage;

// `UTType` lives in UniformTypeIdentifiers; link it so the class is there at run time.
#[link(name = "UniformTypeIdentifiers", kind = "framework")]
unsafe extern "C" {}

/// `UTType(filenameExtension:)?.conforms(to: .image) ?? false`.
fn is_image_extension(ext: &str) -> bool {
    let Some(cls) = AnyClass::get(c"UTType") else {
        return false;
    };
    let ext = NSString::from_str(ext);
    let image_id = NSString::from_str("public.image");
    // SAFETY: `+[UTType typeWithFilenameExtension:]` and `+typeWithIdentifier:` take an NSString
    // and return a UTType or nil; `-conformsToType:` takes a UTType and returns BOOL.
    unsafe {
        let t: Option<Retained<AnyObject>> = msg_send![cls, typeWithFilenameExtension: &*ext];
        let image: Option<Retained<AnyObject>> = msg_send![cls, typeWithIdentifier: &*image_id];
        match (t, image) {
            (Some(t), Some(image)) => msg_send![&*t, conformsToType: &*image],
            _ => false,
        }
    }
}

/// The file URLs on the pasteboard (`readObjects(forClasses: [NSURL.self], options:
/// [.urlReadingFileURLsOnly: true])`), as paths.
fn file_paths(pb: &NSPasteboard) -> Vec<std::path::PathBuf> {
    let classes = NSArray::<AnyClass>::from_slice(&[NSURL::class()]);
    let yes = NSNumber::new_bool(true);
    // SAFETY: an immutable NSString constant exported by AppKit.
    let key = unsafe { NSPasteboardURLReadingFileURLsOnlyKey };
    let value: &AnyObject = yes.as_ref();
    let options = NSDictionary::from_slices(&[key], &[value]);
    // SAFETY: the classes array holds NSURL, the options dictionary maps a reading-option key to
    // an NSNumber, as the API documents; the result holds NSURLs.
    let objects = unsafe { pb.readObjectsForClasses_options(&classes, Some(&options)) };
    let Some(objects) = objects else {
        return Vec::new();
    };
    objects
        .iter()
        .filter_map(|o| o.downcast::<NSURL>().ok())
        .filter_map(|u| u.path().map(|p| std::path::PathBuf::from(p.to_string())))
        .collect()
}

/// `ImageIntake.items(from: .general, preferText: true)` (W/ComposerTextView.swift).
pub fn pasteboard_images(max_file_bytes: u64) -> Option<Vec<PastedImage>> {
    let pb = NSPasteboard::generalPasteboard();
    let paths = file_paths(&pb);
    let images: Vec<_> = paths
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(is_image_extension)
        })
        .collect();
    if !images.is_empty() {
        return Some(
            images
                .into_iter()
                .map(|path| {
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    let data = if size <= max_file_bytes {
                        std::fs::read(path).ok()
                    } else {
                        None
                    };
                    PastedImage {
                        filename,
                        data,
                        size,
                    }
                })
                .collect(),
        );
    }
    if !paths.is_empty() {
        return None;
    }
    // SAFETY: immutable NSString constants exported by AppKit.
    let (string, png, tiff) = unsafe {
        (
            NSPasteboardTypeString,
            NSPasteboardTypePNG,
            NSPasteboardTypeTIFF,
        )
    };
    if pb.stringForType(string).is_some() {
        return None;
    }
    for (kind, filename) in [(png, "Pasted image.png"), (tiff, "Pasted image.tiff")] {
        if let Some(data) = pb.dataForType(kind) {
            let bytes = data.to_vec();
            return Some(vec![PastedImage {
                filename: filename.to_owned(),
                size: bytes.len() as u64,
                data: Some(bytes),
            }]);
        }
    }
    None
}

/// `try data.write(to: url)`; the error is the `NSError`'s `localizedDescription`.
pub fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSData::with_bytes(data)
        .writeToURL_options_error(&url, NSDataWritingOptions::empty())
        .map_err(|e| e.localizedDescription().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn det_34_write_errors_are_foundations_text() {
        let err = write_file(Path::new("/nonexistent-dir/x/vpn.log"), b"hello")
            .expect_err("no such folder");
        assert!(err.contains("vpn.log"), "{err}");
        assert!(!err.contains("os error"), "{err}");
        let dir = std::env::temp_dir().join(format!("itopwrap-files-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("ok.txt");
        assert_eq!(write_file(&path, b"hi"), Ok(()));
        assert_eq!(std::fs::read(&path).ok(), Some(b"hi".to_vec()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn image_extensions_follow_uttype() {
        assert!(is_image_extension("png"));
        assert!(is_image_extension("HEIC"));
        assert!(!is_image_extension("txt"));
    }
}
