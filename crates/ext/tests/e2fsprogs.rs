//! e2fsprogs as the oracle: `e2fsck -fn` must call the emulator's images
//! clean and `dumpe2fs -h` must report the geometry the emulator wrote.
//! Without the tools each test prints `e2fsprogs not found; skipping` and
//! passes, unless `CI` is set, where a missing tool fails the test.
//! Locally: `brew install e2fsprogs` (keg-only; found under its sbin).

mod common;

use common::pattern;
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

/// Task 5: the scripted sequence of spec section 9, judged by `e2fsck` and
/// read back by `debugfs`.
mod scripted_sequence {
    use super::pattern;
    use ext::{ExtFormatOptions, ExtFs};

    /// The `(name, size)` pairs of a `debugfs -R "ls -l DIR"` listing, in
    /// disk order, without `.`, `..`, and unused entries.
    ///
    /// debugfs's long listing (`debugfs/ls.c`) prints one line per
    /// directory entry: `%c%6u%c %6o (%d)  %5d  %5d   ` (bracket, inode,
    /// bracket, octal mode, file type, uid, gid), then the size, then the
    /// modification time as `%2d-%s-%4d %02d:%02d`, then the name:
    /// `     12  100644 (1)      0      0     100  1-Jan-1980 00:00 small.txt`.
    /// Split on whitespace, token 0 is the inode, token 5 the size, and the
    /// last token the name (this test's names contain no spaces; the date's
    /// shape does not matter). The brackets are spaces unless `-d` is
    /// given; they are trimmed anyway. debugfs also lists entries whose
    /// inode is 0 (unused space, including a removed first-in-block entry
    /// that kept its name), with a blank date; they are dropped by their
    /// inode token.
    fn parse_ls_l(listing: &str) -> Vec<(String, u64)> {
        listing
            .lines()
            .filter_map(|line| {
                let tokens: Vec<&str> = line.split_whitespace().collect();
                if tokens.len() < 7 {
                    return None;
                }
                let inode = tokens[0].trim_matches(|c| c == '<' || c == '>');
                let name = *tokens.last()?;
                if inode == "0" || name == "." || name == ".." {
                    return None;
                }
                Some((name.to_string(), tokens[5].parse().ok()?))
            })
            .collect()
    }

    #[test]
    fn parse_ls_l_reads_the_documented_columns() {
        let listing = "      2   40755 (2)      0      0    1024  1-Jan-1980 00:00 .\n\
                       \x20     2   40755 (2)      0      0    1024  1-Jan-1980 00:00 ..\n\
                       \x20    11   40700 (2)      0      0   12288  1-Jan-1980 00:00 lost+found\n\
                       \x20    12  100644 (1)      0      0     100  1-Jan-1980 00:00 small.txt\n\
                       \x20     0       0 (1)      0      0       0                   gone.txt\n";
        assert_eq!(
            parse_ls_l(listing),
            vec![
                ("lost+found".to_string(), 12288),
                ("small.txt".to_string(), 100)
            ]
        );
    }

    /// The `Size:` and `Links:` values of a `debugfs -R "stat PATH"` dump.
    ///
    /// debugfs's `dump_inode` (`debugfs/debugfs.c`) prints the size on the
    /// `User: ... Group: ... Size: N` line and the link count on the
    /// `Links: N   Blockcount: M` line; a later `Fragment: ... Size: 0`
    /// line repeats the `Size:` label, so the first occurrence of each
    /// label is the one read.
    fn parse_stat(dump: &str) -> (Option<u64>, Option<u32>) {
        let tokens: Vec<&str> = dump.split_whitespace().collect();
        let after = |label: &str| {
            tokens
                .windows(2)
                .find(|w| w[0] == label)
                .map(|w| w[1].to_string())
        };
        (
            after("Size:").and_then(|v| v.parse().ok()),
            after("Links:").and_then(|v| v.parse().ok()),
        )
    }

    #[test]
    fn parse_stat_reads_the_first_size_and_links() {
        let dump = "Inode: 14   Type: regular    Mode:  0644   Flags: 0x0\n\
                    Generation: 0    Version: 0x00000000\n\
                    User:     0   Group:     0   Size: 100\n\
                    File ACL: 0\n\
                    Links: 1   Blockcount: 2\n\
                    Fragment:  Address: 0    Number: 0    Size: 0\n";
        assert_eq!(parse_stat(dump), (Some(100), Some(1)));
    }

    /// Names of 9 bytes (20-byte records, 51 to a block): 700 of them
    /// need 14 directory blocks, past the 12 direct pointers.
    const WIDE_ENTRIES: usize = 700;

    #[test]
    fn e2fsck_is_clean_and_debugfs_agrees_after_a_scripted_sequence() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        fs.create_file("/small.txt", &pattern(100)).unwrap();
        fs.create_file("/mid.bin", &pattern(20 * 1024)).unwrap();
        fs.create_file("/big.bin", &pattern(300 * 1024)).unwrap();
        fs.create_file("/indirect.bin", &pattern(20 * 1024))
            .unwrap();
        fs.create_file("/grow.bin", &pattern(1024)).unwrap();
        fs.create_dir("/many").unwrap();
        for i in 0..60 {
            fs.create_file(
                &format!("/many/entry-{i:02}.txt"),
                format!("entry {i}\n").as_bytes(),
            )
            .unwrap();
        }
        // An overwrite that shrinks out of the single-indirect range, with
        // bytes shifted so every kept block changes.
        fs.write_file("/mid.bin", &pattern(5 * 1024 + 7)[7..])
            .unwrap();
        // Growth from one direct block into the double-indirect range.
        fs.write_file("/grow.bin", &pattern(300 * 1024 + 3)[3..])
            .unwrap();
        // A shrink from the double-indirect range to part of one block.
        fs.write_file("/big.bin", &pattern(100)).unwrap();
        // A delete that frees a single-indirect block (20 KiB = 20 data blocks).
        fs.delete_file("/indirect.bin").unwrap();
        // Deletes: the first entry of /many's second block, and one merged into its predecessor.
        fs.delete_file("/many/entry-50.txt").unwrap();
        fs.delete_file("/many/entry-07.txt").unwrap();
        fs.create_dir("/scratch").unwrap();
        fs.remove_dir("/scratch").unwrap();
        // A directory that grows past its 12 direct blocks.
        fs.create_dir("/wide").unwrap();
        for i in 0..WIDE_ENTRIES {
            fs.create_file(&format!("/wide/entry-{i:03}"), b"").unwrap();
        }
        assert!(fs.stat("/wide").unwrap().size > 12 * 1024);
        assert_eq!(fs.stat("/grow.bin").unwrap().size, 300 * 1024);
        assert_eq!(fs.stat("/big.bin").unwrap().size, 100);

        let (Some(e2fsck), Some(debugfs)) = (super::tool("e2fsck"), super::tool("debugfs")) else {
            return;
        };
        let image = super::image_file(&fs, "sequence");
        let fsck = super::run(&e2fsck, &["-fn"], &image);
        let listings: Vec<(&str, std::process::Output)> = ["/", "/many", "/wide"]
            .into_iter()
            .map(|dir| {
                let request = format!("ls -l {dir}");
                (dir, super::run(&debugfs, &["-R", request.as_str()], &image))
            })
            .collect();
        let big = super::run(&debugfs, &["-R", "cat /big.bin"], &image);
        let mid = super::run(&debugfs, &["-R", "cat /mid.bin"], &image);
        let grow = super::run(&debugfs, &["-R", "cat /grow.bin"], &image);
        let big_stat = super::run(&debugfs, &["-R", "stat /big.bin"], &image);
        std::fs::remove_file(&image).unwrap();

        let report = super::both_streams(&fsck);
        assert_eq!(fsck.status.code(), Some(0), "e2fsck -fn:\n{report}");
        assert!(
            !report.contains("Fix?") && !report.contains("FIXED"),
            "e2fsck -fn:\n{report}"
        );
        for (dir, output) in listings {
            assert!(
                output.status.success(),
                "debugfs ls -l {dir}:\n{}",
                super::both_streams(&output)
            );
            let listed = parse_ls_l(&String::from_utf8_lossy(&output.stdout));
            let expected: Vec<(String, u64)> = fs
                .list_dir(dir)
                .unwrap()
                .into_iter()
                .map(|e| (e.name, e.size))
                .collect();
            assert_eq!(listed, expected, "debugfs ls -l {dir}");
        }
        assert!(
            big.stdout == fs.read_file("/big.bin").unwrap(),
            "debugfs cat /big.bin"
        );
        assert!(
            mid.stdout == fs.read_file("/mid.bin").unwrap(),
            "debugfs cat /mid.bin"
        );
        assert!(
            grow.stdout == fs.read_file("/grow.bin").unwrap(),
            "debugfs cat /grow.bin"
        );
        assert_eq!(
            parse_stat(&String::from_utf8_lossy(&big_stat.stdout)),
            (Some(fs.stat("/big.bin").unwrap().size), Some(1)),
            "debugfs stat /big.bin:\n{}",
            super::both_streams(&big_stat)
        );
    }
}

/// ext3 Task 3: a fresh ext3 volume as dumpe2fs, debugfs, and e2fsck see it
/// (ext3 spec section 10.3, first bullet).
mod ext3_format {
    use super::common::ext3;
    use super::{assert_clean, both_streams, image_file, run, tool};
    use ext::{ExtFormatOptions, ExtFs, JournalMode, JournalOptions};

    /// `common::ext3` on a disk of `total_blocks` blocks instead of the
    /// default 16,384, for the journal-size cases.
    fn ext3_with_blocks(total_blocks: u32, mode: JournalMode) -> ExtFs {
        ExtFs::format(ExtFormatOptions {
            total_blocks,
            journal: Some(JournalOptions { blocks: None, mode }),
            ..Default::default()
        })
        .unwrap()
    }

    /// The stdout of `tool_name args image`, one entry per line, with every
    /// run of whitespace collapsed to one space: dumpe2fs pads after the
    /// colon (`Journal inode:            8` becomes `Journal inode: 8`).
    /// `None` when the tool is missing.
    fn normalised(fs: &ExtFs, name: &str, tool_name: &str, args: &[&str]) -> Option<Vec<String>> {
        let program = tool(tool_name)?;
        let image = image_file(fs, name);
        let output = run(&program, args, &image);
        std::fs::remove_file(&image).unwrap();
        assert!(
            output.status.success(),
            "{tool_name} {args:?}:\n{}",
            both_streams(&output)
        );
        Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect(),
        )
    }

    fn assert_lines(lines: &[String], want: &[&str], what: &str) {
        for line in want {
            assert!(
                lines.iter().any(|l| l == line),
                "{what}: no line {line:?} in\n{}",
                lines.join("\n")
            );
        }
    }

    #[test]
    fn dumpe2fs_describes_the_journal_in_both_modes() {
        for (mode, opts) in [
            (JournalMode::Ordered, "journal_data_ordered"),
            (JournalMode::Data, "journal_data"),
        ] {
            let fs = ext3(mode);
            let Some(lines) = normalised(&fs, "ext3-dumpe2fs", "dumpe2fs", &["-h"]) else {
                return;
            };
            let mount = format!("Default mount options: {opts}");
            assert_lines(
                &lines,
                &[
                    "Filesystem features: has_journal filetype sparse_super",
                    mount.as_str(),
                    "Free blocks: 15205",
                    "Free inodes: 1013",
                    "Journal inode: 8",
                    "Journal backup: inode blocks",
                    "Journal features: (none)",
                    "Total journal size: 1024k",
                    "Total journal blocks: 1024",
                    "Max transaction length: 1024",
                    "Journal sequence: 0x00000001",
                    "Journal start: 0",
                ],
                &format!("dumpe2fs -h ({mode:?})"),
            );
        }
    }

    #[test]
    fn debugfs_reads_inode_8_as_the_emulator_wrote_it() {
        let fs = ext3(JournalMode::Ordered);
        let Some(lines) = normalised(&fs, "ext3-stat-8", "debugfs", &["-R", "stat <8>"]) else {
            return;
        };
        let text = lines.join("\n");
        for want in [
            "Type: regular Mode: 0600",
            "Size: 1048576",
            "Links: 1 Blockcount: 2058",
            "(0-11):82-93, (IND):1106, (12-267):94-349, (DIND):1107, (IND):1108, \
             (268-523):350-605, (IND):1109, (524-779):606-861, (IND):1110, (780-1023):862-1105",
            "TOTAL: 1029",
        ] {
            assert!(
                text.contains(want),
                "debugfs stat <8>: no {want:?} in\n{text}"
            );
        }
    }

    #[test]
    fn an_mke2fs_ext3_image_loads_with_the_same_journal_geometry() {
        let Some(mke2fs) = tool("mke2fs") else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("ext2-emulator-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("mke2fs-ext3.img");
        std::fs::write(&image, vec![0u8; 16 * 1024 * 1024]).unwrap();
        let args = [
            "-q",
            "-F",
            "-t",
            "ext3",
            "-b",
            "1024",
            "-I",
            "128",
            "-O",
            "none,has_journal,filetype,sparse_super",
            "-J",
            "size=1",
            "-N",
            "1024",
        ];
        let output = run(&mke2fs, &args, &image);
        let bytes = std::fs::read(&image).unwrap();
        std::fs::remove_file(&image).unwrap();
        assert!(
            output.status.success(),
            "mke2fs:\n{}",
            both_streams(&output)
        );
        let fs = ExtFs::from_image(bytes).unwrap();
        assert_eq!(fs.fs_type(), "ext3");
        assert_eq!(fs.superblock().free_blocks_count, 15_205);
        let info = fs.journal_info().unwrap();
        assert_eq!(
            (info.maxlen, info.first_block, info.sequence, info.start),
            (1_024, 82, 1, 0)
        );
        // mke2fs sets `user_xattr acl` and no journal mode bits: ordered.
        assert_eq!(info.mode, JournalMode::Ordered);
        assert!(!info.needs_recovery);
        let journal_blocks = fs
            .block_owners()
            .values()
            .filter(|o| o.path == "<journal>")
            .count();
        assert_eq!(journal_blocks, 1_029);
    }

    #[test]
    fn e2fsck_calls_both_modes_clean() {
        assert_clean(&ext3(JournalMode::Ordered), "ext3-ordered");
        assert_clean(&ext3(JournalMode::Data), "ext3-data");
    }

    #[test]
    fn a_4096_block_volume_gets_1024_journal_blocks_and_is_clean() {
        let fs = ext3_with_blocks(4_096, JournalMode::Ordered);
        assert_eq!(fs.journal_info().unwrap().maxlen, 1_024);
        assert_clean(&fs, "ext3-4096");
        let Some(lines) = normalised(&fs, "ext3-4096-dumpe2fs", "dumpe2fs", &["-h"]) else {
            return;
        };
        assert_lines(
            &lines,
            &["Total journal blocks: 1024"],
            "dumpe2fs -h (4096)",
        );
    }

    #[test]
    fn a_262144_block_volume_gets_8192_journal_blocks_and_is_clean() {
        let fs = ext3_with_blocks(262_144, JournalMode::Ordered);
        assert_eq!(fs.journal_info().unwrap().maxlen, 8_192);
        assert_clean(&fs, "ext3-262144");
        let Some(lines) = normalised(&fs, "ext3-262144-dumpe2fs", "dumpe2fs", &["-h"]) else {
            return;
        };
        assert_lines(
            &lines,
            &["Total journal blocks: 8192", "Total journal size: 8M"],
            "dumpe2fs -h (262144)",
        );
    }
}
