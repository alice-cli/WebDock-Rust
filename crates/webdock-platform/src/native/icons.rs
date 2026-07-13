//! App icon → `data:image/png;base64,...` for the browser sidebar.
//!
//! Matches Swift `IconCache` behavior (NSWorkspace / .app icns).

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use base64::{engine::general_purpose::STANDARD, Engine};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageBuffer, ImageEncoder};
use parking_lot::Mutex;
#[cfg(target_os = "macos")]
use tracing::debug;

static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve a PNG data-URL for an app path / icon key (cached).
pub fn data_url_for_key(key: &str) -> Option<String> {
    if key.is_empty() || key == "system:display" {
        return display_icon_data_url();
    }
    {
        let g = cache().lock();
        if let Some(hit) = g.get(key) {
            return hit.clone();
        }
    }
    let url = resolve_uncached(key);
    cache().lock().insert(key.to_string(), url.clone());
    url
}

fn resolve_uncached(key: &str) -> Option<String> {
    // 1) Fast path: parse .icns from the .app bundle (bulk app list).
    let path = Path::new(key);
    if let Some(app) = find_app_bundle(path).or_else(|| {
        if path.extension().and_then(|e| e.to_str()) == Some("app") {
            Some(path.to_path_buf())
        } else {
            None
        }
    }) {
        if let Some(icns) = find_icns_in_app(&app) {
            if let Some(png) = icns_file_to_png(&icns, 32) {
                return Some(format!("data:image/png;base64,{}", STANDARD.encode(png)));
            }
        }
    }

    // 2) NSWorkspace covers Assets.car / modern icons (slower; used for window list).
    #[cfg(target_os = "macos")]
    {
        if let Some(url) = macos_nsworkspace_icon(key) {
            return Some(url);
        }
    }
    None
}

/// Best-effort path for a process id (used for window list icons).
pub fn path_for_pid(pid: i32) -> Option<String> {
    if pid <= 0 {
        return None;
    }
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    let spid = Pid::from_u32(pid as u32);
    sys.refresh_processes(ProcessesToUpdate::Some(&[spid]), true);
    let proc = sys.process(spid)?;
    let exe = proc.exe()?;
    let app = find_app_bundle(exe).unwrap_or_else(|| exe.to_path_buf());
    Some(app.to_string_lossy().into_owned())
}

fn find_app_bundle(path: &Path) -> Option<PathBuf> {
    let mut p = path.to_path_buf();
    loop {
        if p.extension().and_then(|e| e.to_str()) == Some("app") && p.is_dir() {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

fn find_icns_in_app(app: &Path) -> Option<PathBuf> {
    let resources = app.join("Contents/Resources");
    if !resources.is_dir() {
        return None;
    }
    // Info.plist CFBundleIconFile
    if let Some(name) = read_bundle_icon_name(app) {
        let candidates = [
            resources.join(format!("{name}.icns")),
            resources.join(&name),
        ];
        for c in candidates {
            if c.is_file() {
                return Some(c);
            }
        }
    }
    for preferred in ["AppIcon.icns", "app.icns", "Icon.icns", "application.icns"] {
        let p = resources.join(preferred);
        if p.is_file() {
            return Some(p);
        }
    }
    // First .icns in Resources
    let rd = std::fs::read_dir(&resources).ok()?;
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("icns") {
            return Some(p);
        }
    }
    None
}

fn read_bundle_icon_name(app: &Path) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    let text = std::fs::read_to_string(&plist).ok()?;
    // Very small XML/plist scanner for CFBundleIconFile / CFBundleIconName
    for key in ["CFBundleIconFile", "CFBundleIconName"] {
        if let Some(v) = plist_string_value(&text, key) {
            return Some(v);
        }
    }
    None
}

fn plist_string_value(plist: &str, key: &str) -> Option<String> {
    // <key>CFBundleIconFile</key>\n\t<string>AppIcon</string>
    let needle = format!("<key>{key}</key>");
    let idx = plist.find(&needle)?;
    let rest = &plist[idx + needle.len()..];
    let start = rest.find("<string>")? + 8;
    let end = rest[start..].find("</string>")? + start;
    let s = rest[start..end].trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn icns_file_to_png(path: &Path, size: u32) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    let family = icns::IconFamily::read(Cursor::new(&data)).ok()?;
    // Prefer sizes near target
    let types = [
        icns::IconType::RGBA32_32x32,
        icns::IconType::RGBA32_32x32_2x,
        icns::IconType::RGBA32_64x64,
        icns::IconType::RGBA32_128x128,
        icns::IconType::RGBA32_16x16,
        icns::IconType::RGBA32_256x256,
        icns::IconType::RGB24_32x32,
        icns::IconType::RGB24_16x16,
    ];
    let mut image = None;
    for t in types {
        if let Ok(img) = family.get_icon_with_type(t) {
            image = Some(img);
            break;
        }
    }
    // Prefer any available icon if preferred types missing.
    if image.is_none() {
        for t in family.available_icons() {
            if let Ok(img) = family.get_icon_with_type(t) {
                image = Some(img);
                break;
            }
        }
    }
    let img = image?;
    let w = img.width();
    let h = img.height();
    let raw = img.data();
    let bpp = match img.pixel_format() {
        icns::PixelFormat::RGBA => 4,
        icns::PixelFormat::RGB => 3,
        icns::PixelFormat::GrayAlpha => 2,
        icns::PixelFormat::Gray => 1,
        _ => 4,
    };
    let rgba = match bpp {
        4 if raw.len() >= (w * h * 4) as usize => raw[..(w * h * 4) as usize].to_vec(),
        3 if raw.len() >= (w * h * 3) as usize => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for chunk in raw.chunks(3).take((w * h) as usize) {
                out.extend_from_slice(chunk);
                out.push(255);
            }
            out
        }
        1 if raw.len() >= (w * h) as usize => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for &g in raw.iter().take((w * h) as usize) {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        2 if raw.len() >= (w * h * 2) as usize => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for chunk in raw.chunks(2).take((w * h) as usize) {
                out.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            out
        }
        _ => return None,
    };
    let buf: image::RgbaImage = ImageBuffer::from_raw(w, h, rgba)?;
    let resized = if w != size || h != size {
        image::imageops::resize(&buf, size, size, image::imageops::FilterType::Triangle)
    } else {
        buf
    };
    let mut out = Cursor::new(Vec::new());
    let enc = PngEncoder::new(&mut out);
    enc.write_image(resized.as_raw(), size, size, ColorType::Rgba8.into())
        .ok()?;
    Some(out.into_inner())
}

#[cfg(target_os = "macos")]
fn macos_nsworkspace_icon(path: &str) -> Option<String> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSString};

    autoreleasepool(|_| {
        let ns_path = NSString::from_str(path);
        // If path is inside .app, use bundle root for better icon.
        let path_obj = {
            let p = Path::new(path);
            if let Some(app) = find_app_bundle(p) {
                NSString::from_str(&app.to_string_lossy())
            } else {
                ns_path
            }
        };
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let image = unsafe { workspace.iconForFile(&path_obj) };
        // Force 32pt
        unsafe {
            image.setSize(objc2_foundation::NSSize {
                width: 32.0,
                height: 32.0,
            });
        }
        let tiff = unsafe { image.TIFFRepresentation() }?;
        let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
        let props = NSDictionary::new();
        let png =
            unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) }?;
        let bytes = png.to_vec();
        if bytes.is_empty() {
            return None;
        }
        debug!(path, bytes = bytes.len(), "nsworkspace icon");
        Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    })
}

/// Tiny built-in display glyph (16×16 blue monitor-ish PNG) as data URL.
fn display_icon_data_url() -> Option<String> {
    // 1x1 transparent is useless; generate a simple 32x32 PNG programmatically.
    let size = 32u32;
    let mut img = image::RgbaImage::new(size, size);
    for y in 0..size {
        for x in 0..size {
            // rounded rect body
            let border = x < 2 || y < 2 || x >= size - 2 || y >= size - 2;
            let stand = y > 24 && x > 10 && x < 22;
            let base = y > 28 && x > 6 && x < 26;
            let c = if border || stand || base {
                image::Rgba([90u8, 140, 220, 255])
            } else if y > 4 && y < 22 && x > 4 && x < 28 {
                image::Rgba([40, 50, 70, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            };
            img.put_pixel(x, y, c);
        }
    }
    let mut out = Cursor::new(Vec::new());
    let enc = PngEncoder::new(&mut out);
    enc.write_image(img.as_raw(), size, size, ColorType::Rgba8.into())
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(out.into_inner())
    ))
}
