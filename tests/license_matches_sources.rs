//! `LICENSE` at this crate's root is a hand-maintained combination of
//! breadpaper's own MIT text, `NOTICE`, and the ImageMagick and
//! libjpeg-turbo/IJG licence texts: breadpaper always builds bread-theme
//! with its `extract` feature enabled, so it always statically links
//! bread-palette-extract's ported code. `bakery.toml`'s `license_file`
//! points at this one file, and the release workflows copy it verbatim
//! into the shipped package, so it's the only licence text an end user
//! actually receives (mirroring how bread-theme's own combined LICENSE
//! works). Nothing about the build enforces that it stays in sync with its
//! sources; this test catches the drift the moment `LICENSE-ImageMagick` or
//! `LICENSE-libjpeg-turbo` changes and `LICENSE` is not regenerated to
//! match.

use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

#[test]
fn license_starts_with_breadpapers_own_mit_text() {
    let combined = read("LICENSE");
    assert!(
        combined.starts_with("MIT License"),
        "LICENSE must lead with breadpaper's own MIT licence"
    );
    assert!(
        combined.contains("THE SOFTWARE IS PROVIDED \"AS IS\""),
        "LICENSE is missing the standard MIT disclaimer text"
    );
}

#[test]
fn license_carries_the_imagemagick_license_verbatim() {
    let combined = read("LICENSE");
    let source = read("LICENSE-ImageMagick");
    assert!(
        combined.contains(source.trim()),
        "LICENSE has drifted from LICENSE-ImageMagick: regenerate the combined \
         LICENSE from its sources"
    );
}

#[test]
fn license_carries_the_libjpeg_turbo_license_verbatim() {
    let combined = read("LICENSE");
    let source = read("LICENSE-libjpeg-turbo");
    assert!(
        combined.contains(source.trim()),
        "LICENSE has drifted from LICENSE-libjpeg-turbo: regenerate the combined \
         LICENSE from its sources"
    );
    // Clause 2 of the IJG License: an executable-only distribution's
    // documentation must carry this exact sentence, and this file (plus
    // `breadpaper notice`) is the only place it reaches the user.
    assert!(
        combined.contains("This software is based in part on the work of the Independent JPEG"),
        "LICENSE is missing the IJG attribution sentence required for an \
         executable-only distribution"
    );
}

#[test]
fn license_carries_the_notice_text() {
    let combined = read("LICENSE");
    let notice = read("NOTICE");
    // NOTICE's own first two lines (crate name + "Licensed under the MIT
    // License") duplicate what the MIT header above already states, so
    // only the body from the third line on is expected to appear verbatim.
    let body: String = notice
        .splitn(3, '\n')
        .nth(2)
        .expect("NOTICE must have a body after its two-line header")
        .to_string();
    assert!(
        combined.contains(body.trim()),
        "LICENSE has drifted from NOTICE's body: regenerate the combined LICENSE"
    );
}
