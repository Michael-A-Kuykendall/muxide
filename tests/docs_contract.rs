use std::fs;

fn read_repo(name: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    fs::read_to_string(format!("{root}/{name}")).unwrap_or_else(|err| panic!("read {name}: {err}"))
}

#[test]
fn docs_msrv_matches_cargo() {
    let cargo = read_repo("Cargo.toml");
    let readme = read_repo("README.md");
    let contributing = read_repo("CONTRIBUTING.md");

    assert!(cargo.contains("rust-version = \"1.86\""));
    assert!(readme.contains("MSRV-1.86"));
    assert!(contributing.contains("MSRV 1.86"));
    assert!(!contributing.contains("No unsafe, no FFI"));
    assert!(
        readme.contains(".frames"),
        "README must mention the .frames CLI input"
    );
}

/// Pins the 0.5.0 documentation to the playback contract.
/// A stale "video is required" sentence, or a roadmap that files this work under 0.4.0, fails here.
#[test]
fn docs_0_5_0_matches_playback_contract() {
    let cargo = read_repo("Cargo.toml");
    let changelog = read_repo("CHANGELOG.md");
    let contract = read_repo("docs/contract.md");
    let charter = read_repo("docs/charter.md");
    let roadmap = read_repo("ROADMAP.md");
    let readme = read_repo("README.md");
    let zh_cn = read_repo("docs/zh-CN/README.md");
    let zh_tw = read_repo("docs/zh-TW/README.md");
    let header = read_repo("bindings/muxide.h");
    let guide = read_repo("docs/INVARIANT_PPT_GUIDE.md");

    assert!(
        cargo.contains("version = \"0.5.0\""),
        "Cargo.toml must be 0.5.0"
    );
    assert!(changelog.contains("## 0.5.0"));
    assert!(
        changelog.contains("GitHub issue #2"),
        "changelog must record the audio-only fMP4 track-id fix"
    );
    assert!(changelog.contains("track 1 when the muxer has no video track"));

    assert!(contract.starts_with("# Muxide API Contract (v0.5.0)\n"));
    assert!(contract.contains("MissingTrackConfig"));
    assert!(contract.contains("audio-only"));
    assert!(contract.contains("1..=65535"));
    assert!(contract.contains("VideoTrackNotConfigured"));
    assert!(contract.contains("new_audio_only"));
    assert!(contract.contains("data_offset"));
    assert!(
        !contract.contains("A video track must have been configured"),
        "contract must not require a video track"
    );
    assert!(
        !contract.contains("the absence of a video track is an error"),
        "audio-only files are valid"
    );
    assert!(!contract.contains("v0.2.x"));

    assert!(charter.contains("audio-only"));
    assert!(
        !charter.contains("one video track per file"),
        "charter must not present one video track as the current contract"
    );

    assert!(roadmap.contains("### v0.5.0 — Playback correctness"));
    assert!(roadmap.contains("GitHub issue #2"));
    assert!(
        !roadmap.contains("gated behind `debug_assertions`"),
        "assert_invariant! is not debug-only"
    );
    let v040 = roadmap
        .split("### v0.4.0")
        .nth(1)
        .expect("v0.4.0 section")
        .split("### v0.5.0")
        .next()
        .expect("v0.5.0 follows v0.4.0");
    assert!(
        !v040.contains("DASH") && !v040.to_ascii_lowercase().contains("two-track"),
        "two-track fMP4 and DASH belong under v0.5.0, not v0.4.0: {v040}"
    );

    assert!(readme.contains("Audio-only"));
    assert!(readme.contains("new_audio_only"));
    assert!(readme.contains("--sample-rate"));
    assert!(zh_cn.contains("仅音频"));
    assert!(zh_tw.contains("僅音頻"));
    assert!(header.contains("Returns NULL when width or height is outside 1..=65535"));
    assert!(guide.contains("including in release builds"));
}
