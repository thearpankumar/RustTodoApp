//! Renders every packaging icon the project needs from the single vector source
//! `assets/logo.svg`, so Windows, Linux and macOS can never drift apart.

use std::{
    fs::{self, File},
    io::BufWriter,
    path::Path,
};

use resvg::{tiny_skia, usvg};

/// Sizes dropped into `assets/icons/`, matching the `hicolor` theme directories
/// the Linux packages install into.
const HICOLOR_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256, 512];

/// Sizes embedded in the multi-resolution Windows `.ico`.
const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

/// Sizes embedded in the macOS `.icns`. Only the pixel sizes Apple defines an
/// icon type for are valid here.
const ICNS_SIZES: &[u32] = &[16, 32, 64, 128, 256, 512, 1024];

pub fn run(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = root.join("assets/logo.svg");
    let svg = fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;

    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default())?;

    let assets = root.join("assets");
    let icons_dir = assets.join("icons");
    fs::create_dir_all(&icons_dir)?;

    // Every size we will ever need, rendered once.
    let mut wanted: Vec<u32> = HICOLOR_SIZES
        .iter()
        .chain(ICO_SIZES)
        .chain(ICNS_SIZES)
        .copied()
        .collect();
    wanted.sort_unstable();
    wanted.dedup();

    let rendered: Vec<(u32, tiny_skia::Pixmap)> = wanted
        .into_iter()
        .map(|size| render(&tree, size).map(|pixmap| (size, pixmap)))
        .collect::<Result<_, _>>()?;

    let pixmap = |size: u32| -> &tiny_skia::Pixmap {
        &rendered
            .iter()
            .find(|(s, _)| *s == size)
            .expect("size was rendered")
            .1
    };

    // 1. Individual PNGs for the Linux icon theme.
    for &size in HICOLOR_SIZES {
        let path = icons_dir.join(format!("{size}x{size}.png"));
        pixmap(size).save_png(&path)?;
        println!("  {}", path.display());
    }

    // 2. The 512px PNG the app embeds as its window icon and the packages drop
    //    into /usr/share/pixmaps.
    let icon_png = assets.join("icon.png");
    pixmap(512).save_png(&icon_png)?;
    println!("  {}", icon_png.display());

    // 3. Windows .ico (also embedded into the .exe by build.rs).
    let mut ico_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in ICO_SIZES {
        let image = ico::IconImage::from_rgba_data(size, size, pixmap(size).data().to_vec());
        ico_dir.add_entry(ico::IconDirEntry::encode(&image)?);
    }
    let icon_ico = assets.join("icon.ico");
    ico_dir.write(BufWriter::new(File::create(&icon_ico)?))?;
    println!("  {}", icon_ico.display());

    // 4. macOS .icns for the .app bundle.
    let mut family = icns::IconFamily::new();
    for &size in ICNS_SIZES {
        let image = icns::Image::from_data(
            icns::PixelFormat::RGBA,
            size,
            size,
            pixmap(size).data().to_vec(),
        )?;
        family.add_icon(&image)?;
    }
    let icon_icns = assets.join("icon.icns");
    family.write(BufWriter::new(File::create(&icon_icns)?))?;
    println!("  {}", icon_icns.display());

    Ok(())
}

fn render(tree: &usvg::Tree, size: u32) -> Result<tiny_skia::Pixmap, Box<dyn std::error::Error>> {
    let mut pixmap =
        tiny_skia::Pixmap::new(size, size).ok_or_else(|| format!("bad pixmap size {size}"))?;

    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    Ok(pixmap)
}
