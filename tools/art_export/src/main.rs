//! Wandelt alle SVG-Dateien unter `art_src/` in PNG-Dateien unter `assets/` um.
//! Die Ordnerstruktur bleibt erhalten: `art_src/figuren/kaninchen/kopf.svg`
//! wird zu `assets/figuren/kaninchen/kopf.png`.
//!
//! Aufruf aus dem Projektordner: `cargo run -p art_export`

use std::fs;
use std::path::{Path, PathBuf};

use resvg::{tiny_skia, usvg};

fn main() {
    let source = Path::new("art_src");
    let target = Path::new("assets");

    let mut files = Vec::new();
    collect_svgs(source, &mut files);
    if files.is_empty() {
        eprintln!("Keine SVG-Dateien in {} gefunden.", source.display());
        return;
    }

    let mut failed = 0;
    for svg in &files {
        let png = target.join(svg.strip_prefix(source).unwrap()).with_extension("png");
        match export(svg, &png) {
            Ok(()) => println!("{} -> {}", svg.display(), png.display()),
            Err(error) => {
                failed += 1;
                eprintln!("FEHLER {}: {error}", svg.display());
            }
        }
    }
    println!("{} von {} Dateien exportiert.", files.len() - failed, files.len());
    if failed > 0 {
        std::process::exit(1);
    }
}

fn collect_svgs(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_svgs(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "svg") {
            files.push(path);
        }
    }
}

/// Die SVG-Größe ist die PNG-Größe in Pixeln: Teile werden in doppelter Spielgröße gezeichnet.
fn export(svg: &Path, png: &Path) -> Result<(), String> {
    let data = fs::read(svg).map_err(|e| e.to_string())?;
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("SVG hat keine Größe")?;
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());

    if let Some(parent) = png.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    pixmap.save_png(png).map_err(|e| e.to_string())
}
