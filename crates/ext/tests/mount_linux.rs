//! Manual validation: loop-mount the exported image read-only with Linux
//! and read files back, and let the kernel replay a crashed ext3 image.
//! Run with `cargo test -p ext --test mount_linux -- --ignored` as root or
//! with passwordless sudo.
#![cfg(target_os = "linux")]

mod common;

use common::{assert_images_agree, mask_journal};
use ext::{CrashPhase, ExtFormatOptions, ExtFs};
use std::process::Command;

/// `program` as root: directly when already root, else through `sudo -n`,
/// which fails rather than prompting for a password.
fn as_root(program: &str) -> Command {
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if uid == "0" {
        Command::new(program)
    } else {
        let mut sudo = Command::new("sudo");
        sudo.args(["-n", program]);
        sudo
    }
}

#[test]
#[ignore]
fn linux_mounts_the_image_and_reads_files() {
    let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
    fs.create_dir("/docs").unwrap();
    fs.create_file("/docs/hello.txt", b"hello from the emulator\n")
        .unwrap();
    let big: Vec<u8> = (0..300 * 1024).map(|i: usize| (i % 251) as u8).collect();
    fs.create_file("/big.bin", &big).unwrap();

    let dir = std::env::temp_dir().join(format!("ext2-emulator-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let image = dir.join("test.img");
    let mount = dir.join("mnt");
    std::fs::create_dir_all(&mount).unwrap();
    std::fs::write(&image, fs.disk().as_bytes()).unwrap();

    let attach = as_root("mount")
        .args(["-t", "ext2", "-o", "loop,ro"])
        .arg(&image)
        .arg(&mount)
        .output()
        .unwrap();
    assert!(
        attach.status.success(),
        "mount failed: {}",
        String::from_utf8_lossy(&attach.stderr)
    );

    let result = (|| {
        let hello = std::fs::read(mount.join("docs").join("hello.txt"))?;
        let big = std::fs::read(mount.join("big.bin"))?;
        Ok::<_, std::io::Error>((hello, big))
    })();

    let detach = as_root("umount").arg(&mount).output().unwrap();
    assert!(
        detach.status.success(),
        "umount failed: {}",
        String::from_utf8_lossy(&detach.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);

    let (hello, read_big) = result.unwrap();
    assert_eq!(hello, b"hello from the emulator\n");
    assert!(read_big == big, "big.bin differs after the mount");
}

/// ext3 spec section 10.3, last bullet: a read-write mount replays the
/// journal of a crashed after-commit image; after the unmount the image
/// must equal what `recover()` writes under the spec's masking: both images
/// pass through `mask_journal` (the journal's blocks and the journal fields
/// of the superblocks), and the superblock copies may differ in the fields
/// of `SUPERBLOCK_IGNORED`. Unlike the e2fsck oracle, the journal
/// superblock is not compared byte for byte: the kernel's unmount rewrites
/// its fields in its own way.
#[test]
#[ignore]
fn linux_replays_a_crashed_ext3_image_like_recover() {
    let mut fs = ExtFs::format(ExtFormatOptions::ext3()).unwrap();
    fs.create_dir("/docs").unwrap();
    fs.arm_crash(CrashPhase::AfterCommit).unwrap();
    fs.create_file("/docs/hello.txt", b"hello from the emulator\n")
        .unwrap();
    assert!(fs.needs_recovery());

    let dir = std::env::temp_dir().join(format!("ext3-emulator-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let image = dir.join("crashed.img");
    let mount = dir.join("mnt");
    std::fs::create_dir_all(&mount).unwrap();
    std::fs::write(&image, fs.disk().as_bytes()).unwrap();

    let attach = as_root("mount")
        .args(["-t", "ext3", "-o", "loop,rw"])
        .arg(&image)
        .arg(&mount)
        .output()
        .unwrap();
    assert!(
        attach.status.success(),
        "mount failed: {}",
        String::from_utf8_lossy(&attach.stderr)
    );
    let detach = as_root("umount").arg(&mount).output().unwrap();
    assert!(
        detach.status.success(),
        "umount failed: {}",
        String::from_utf8_lossy(&detach.stderr)
    );
    let mut theirs = std::fs::read(&image).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    fs.recover().unwrap();
    let mut ours = fs.disk().as_bytes().to_vec();
    mask_journal(&mut ours, &fs);
    mask_journal(&mut theirs, &fs);
    assert_images_agree(&ours, &theirs, fs.geometry(), "the kernel's");
    assert_eq!(
        fs.read_file("/docs/hello.txt").unwrap(),
        b"hello from the emulator\n"
    );
}
