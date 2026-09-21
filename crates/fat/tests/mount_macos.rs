//! Manual validation: mount the exported image with macOS and read a file
//! back. Run with `cargo test -p fat --test mount_macos -- --ignored`.

use fat::{FatFs, FormatOptions};
use std::process::Command;

#[test]
#[ignore]
fn macos_mounts_the_image_and_reads_files() {
    let mut fs = FatFs::format(FormatOptions::default()).unwrap();
    fs.create_dir("/DOCS").unwrap();
    fs.create_file("/DOCS/Hello world.txt", b"hello from the emulator\n")
        .unwrap();
    fs.create_file("/README.TXT", b"short name\n").unwrap();

    let dir = std::env::temp_dir().join(format!("fat16-emulator-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let image = dir.join("test.img");
    let mount = dir.join("mnt");
    std::fs::create_dir_all(&mount).unwrap();
    std::fs::write(&image, fs.disk().as_bytes()).unwrap();

    let attach = Command::new("hdiutil")
        .args([
            "attach",
            "-imagekey",
            "diskimage-class=CRawDiskImage",
            "-nobrowse",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(&image)
        .output()
        .unwrap();
    assert!(
        attach.status.success(),
        "hdiutil attach failed: {}",
        String::from_utf8_lossy(&attach.stderr)
    );

    let result = (|| {
        let hello = std::fs::read(mount.join("DOCS").join("Hello world.txt"))?;
        let readme = std::fs::read(mount.join("README.TXT"))?;
        Ok::<_, std::io::Error>((hello, readme))
    })();

    let detach = Command::new("hdiutil")
        .arg("detach")
        .arg(&mount)
        .output()
        .unwrap();
    assert!(
        detach.status.success(),
        "hdiutil detach failed: {}",
        String::from_utf8_lossy(&detach.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);

    let (hello, readme) = result.unwrap();
    assert_eq!(hello, b"hello from the emulator\n");
    assert_eq!(readme, b"short name\n");
}
