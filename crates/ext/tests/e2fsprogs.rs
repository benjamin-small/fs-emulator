//! e2fsprogs as the oracle: `e2fsck -fn` must call the emulator's images
//! clean and `dumpe2fs -h` must report the geometry the emulator wrote.
//! Without the tools each test prints `e2fsprogs not found; skipping` and
//! passes, unless `CI` is set, where a missing tool fails the test.
//! Locally: `brew install e2fsprogs` (keg-only; found under its sbin).

use ext::{ExtFormatOptions, ExtFs};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Output};

/// Where e2fsprogs lives when `PATH` misses it: Homebrew's keg-only prefix
/// on Apple silicon, then on Intel, then the sbin directories a non-root
/// `PATH` may omit.
const SBIN_DIRS: [&str; 4] = [
    "/opt/homebrew/opt/e2fsprogs/sbin",
    "/usr/local/opt/e2fsprogs/sbin",
    "/usr/sbin",
    "/sbin",
];

/// The e2fsprogs tool `name`, looked up on `PATH`, then in `SBIN_DIRS`.
/// `None` (after printing the skip notice) when absent; panics instead
/// under `CI`.
fn tool(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let found = std::env::split_paths(&path)
        .chain(SBIN_DIRS.map(PathBuf::from))
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file());
    if found.is_none() {
        assert!(
            std::env::var_os("CI").is_none(),
            "{name} not found on PATH or in {SBIN_DIRS:?}; CI must install e2fsprogs"
        );
        println!("e2fsprogs not found; skipping");
    }
    found
}

/// `len` bytes of the repeating pattern `i % 251`, for file contents the
/// mutation oracle tests of Tasks 4 and 5 write.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// Write the volume's image to a fresh file under the temp directory.
fn image_file(fs: &ExtFs, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ext2-emulator-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{name}.img"));
    std::fs::write(&file, fs.disk().as_bytes()).unwrap();
    file
}

fn run(tool: &PathBuf, args: &[&str], image: &PathBuf) -> Output {
    Command::new(tool).args(args).arg(image).output().unwrap()
}

fn both_streams(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// `e2fsck -fn` must exit 0 and print no fix prompts.
fn assert_clean(fs: &ExtFs, name: &str) {
    let Some(e2fsck) = tool("e2fsck") else {
        return;
    };
    let image = image_file(fs, name);
    let output = run(&e2fsck, &["-fn"], &image);
    let text = both_streams(&output);
    std::fs::remove_file(&image).unwrap();
    assert_eq!(output.status.code(), Some(0), "e2fsck -fn:\n{text}");
    assert!(
        !text.contains("Fix?") && !text.contains("FIXED"),
        "e2fsck -fn:\n{text}"
    );
}

#[test]
fn e2fsck_calls_the_default_format_clean() {
    assert_clean(
        &ExtFs::format(ExtFormatOptions::default()).unwrap(),
        "default",
    );
}

#[test]
fn e2fsck_calls_a_one_group_volume_clean() {
    let fs = ExtFs::format(ExtFormatOptions {
        total_blocks: 64,
        ..Default::default()
    })
    .unwrap();
    assert_clean(&fs, "tiny");
}

#[test]
fn dumpe2fs_reports_the_geometry_the_emulator_wrote() {
    let Some(dumpe2fs) = tool("dumpe2fs") else {
        return;
    };
    let mut label = [0u8; 16];
    label[..8].copy_from_slice(b"emulator");
    let fs = ExtFs::format(ExtFormatOptions {
        label,
        ..Default::default()
    })
    .unwrap();
    let image = image_file(&fs, "dumpe2fs");
    let output = run(&dumpe2fs, &["-h"], &image);
    std::fs::remove_file(&image).unwrap();
    assert!(
        output.status.success(),
        "dumpe2fs -h:\n{}",
        both_streams(&output)
    );
    let fields: BTreeMap<String, String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect();
    for (key, want) in [
        ("Filesystem volume name", "emulator"),
        ("Filesystem magic number", "0xEF53"),
        ("Filesystem features", "filetype sparse_super"),
        ("Block count", "16384"),
        ("Inode count", "1024"),
        ("Free blocks", "16234"),
        ("Free inodes", "1013"),
        ("Block size", "1024"),
        ("Blocks per group", "8192"),
        ("Inodes per group", "512"),
        ("Inode size", "128"),
    ] {
        assert_eq!(
            fields.get(key).map(String::as_str),
            Some(want),
            "dumpe2fs field {key:?}"
        );
    }
}

/// Task 4: images after creates stay clean under `e2fsck -fn`.
mod after_creates {
    use super::{assert_clean, pattern};
    use ext::{ExtFormatOptions, ExtFs};

    #[test]
    fn files_and_directories_on_the_default_disk_are_clean() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        fs.create_file("/hello.txt", &pattern(100)).unwrap();
        fs.create_file("/mid.bin", &pattern(20 * 1024)).unwrap();
        fs.create_file("/big.bin", &pattern(300 * 1024)).unwrap();
        fs.create_dir("/dir").unwrap();
        fs.create_file("/dir/inner.txt", b"inner").unwrap();
        fs.create_dir("/dir/sub").unwrap();
        fs.create_file("/empty", b"").unwrap();
        fs.create_dir("/names").unwrap();
        for i in 0..60 {
            fs.create_file(&format!("/names/entry-with-a-longer-name-{i:02}"), b"")
                .unwrap();
        }
        assert_clean(&fs, "creates-default");
    }

    #[test]
    fn a_directory_placed_in_group_1_is_clean() {
        let mut fs = ExtFs::format(ExtFormatOptions {
            inodes_per_group: Some(16),
            ..Default::default()
        })
        .unwrap();
        for i in 0..5 {
            fs.create_file(&format!("/f{i}"), b"").unwrap();
        }
        fs.create_dir("/dir").unwrap();
        fs.create_file("/dir/inner.bin", &pattern(3000)).unwrap();
        fs.create_file("/late.txt", b"late").unwrap();
        assert_clean(&fs, "creates-group-1");
    }

    #[test]
    fn a_64_block_disk_filled_to_the_last_block_and_inode_is_clean() {
        let mut fs = ExtFs::format(ExtFormatOptions {
            total_blocks: 64,
            ..Default::default()
        })
        .unwrap();
        fs.create_file("/big", &pattern(43 * 1024)).unwrap();
        for i in 0..3 {
            fs.create_file(&format!("/{}{i}", "n".repeat(254)), b"")
                .unwrap();
        }
        fs.create_file("/last", b"").unwrap();
        assert_clean(&fs, "creates-tiny-full");
    }
}
