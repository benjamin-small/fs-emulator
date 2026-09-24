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

    /// The scripted sequence of spec section 9 on `fs`: `/big.bin` and
    /// `/grow.bin` reach `big` bytes (300 KiB on ext2; a data-mode ext3
    /// journal takes at most 256 blocks per transaction, so less there).
    pub(super) fn run_script(fs: &mut ExtFs, big: usize) {
        fs.create_file("/small.txt", &pattern(100)).unwrap();
        fs.create_file("/mid.bin", &pattern(20 * 1024)).unwrap();
        fs.create_file("/big.bin", &pattern(big)).unwrap();
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
        // Growth from one direct block into the double-indirect range
        // (for `big` above 268 KiB).
        fs.write_file("/grow.bin", &pattern(big + 3)[3..]).unwrap();
        // A shrink from the double-indirect range (for `big` above 268 KiB)
        // to part of one block.
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
        assert_eq!(fs.stat("/grow.bin").unwrap().size, big as u64);
        assert_eq!(fs.stat("/big.bin").unwrap().size, 100);
    }

    /// `e2fsck -fn` is clean and `debugfs` lists, reads, and stats what
    /// `fs` does (skipped without the tools).
    pub(super) fn assert_tools_agree(fs: &ExtFs, name: &str) {
        let (Some(e2fsck), Some(debugfs)) = (super::tool("e2fsck"), super::tool("debugfs")) else {
            return;
        };
        let image = super::image_file(fs, name);
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

    #[test]
    fn e2fsck_is_clean_and_debugfs_agrees_after_a_scripted_sequence() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        run_script(&mut fs, 300 * 1024);
        assert_tools_agree(&fs, "sequence");
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

/// ext3 Task 4: journaled mutations as e2fsck and debugfs see them (ext3
/// spec section 10.3, second bullet, and the e2fsck half of the third).
mod ext3_transactions {
    use super::common::ext3;
    use super::scripted_sequence::{assert_tools_agree, run_script};
    use super::{both_streams, image_file, pattern, run, tool};
    use ext::{decode_descriptor, ExtFs, JournalMode};
    use fs_core::OpRecord;

    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// Journal index `i` is physical block `82 + i` on the default disk.
    const JOURNAL_BLOCK_0: usize = 82;

    /// The lines `debugfs -R "logdump -a"` prints for transaction `tid` as
    /// `rec` wrote it: the descriptor, one line per tag, and the commit.
    /// The indexes and flags come from the record's bytes: the descriptor
    /// is the first log write, the copies follow it, the commit follows
    /// them.
    pub(super) fn logdump_lines(rec: &OpRecord, tid: u32, with_commit: bool) -> Vec<String> {
        let log = |offset: usize| {
            (offset / 1024)
                .checked_sub(JOURNAL_BLOCK_0)
                .filter(|index| (1..1024).contains(index))
        };
        let writes: Vec<(usize, &[u8])> = rec
            .changes
            .iter()
            .filter(|c| c.after.len() == 1024)
            .filter_map(|c| log(c.offset).map(|i| (i, c.after.as_slice())))
            .collect();
        let (descriptor, bytes) = writes[0];
        let tags = decode_descriptor(bytes).unwrap();
        let mut lines = vec![format!(
            "Found expected sequence {tid}, type 1 (descriptor block) at block {descriptor}"
        )];
        for (tag, (index, _)) in tags.iter().zip(&writes[1..]) {
            lines.push(format!(
                "FS block {} logged at journal block {index} (flags 0x{:x})",
                tag.block, tag.flags
            ));
        }
        if with_commit {
            lines.push(format!(
                "Found expected sequence {tid}, type 2 (commit block) at block {}",
                writes[tags.len() + 1].0
            ));
        }
        lines
    }

    /// What `debugfs -R "<request>"` prints about transaction `tid`: its
    /// descriptor line, the tag lines that follow it, and its commit line,
    /// trimmed. `None` without debugfs.
    pub(super) fn logdump(fs: &ExtFs, name: &str, request: &str, tid: u32) -> Option<Vec<String>> {
        let debugfs = tool("debugfs")?;
        let image = image_file(fs, name);
        let output = run(&debugfs, &["-R", request], &image);
        std::fs::remove_file(&image).unwrap();
        assert!(
            output.status.success(),
            "{request}:\n{}",
            both_streams(&output)
        );
        let descriptor = format!("Found expected sequence {tid}, type 1 (descriptor block)");
        let commit = format!("Found expected sequence {tid}, type 2 (commit block)");
        let mut lines = Vec::new();
        let mut in_tags = false;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.starts_with(&descriptor) {
                lines.push(line.to_string());
                in_tags = true;
            } else if in_tags && line.starts_with("FS block ") {
                lines.push(line.to_string());
            } else if line.starts_with(&commit) {
                lines.push(line.to_string());
                break;
            } else if !line.starts_with("Dumping descriptor block") {
                in_tags = false;
            }
        }
        Some(lines)
    }

    #[test]
    fn the_scripted_sequence_is_clean_and_debugfs_agrees_in_both_modes() {
        for (mode, big) in [
            (JournalMode::Ordered, 300 * 1024),
            (JournalMode::Data, 200 * 1024),
        ] {
            let mut fs = ext3(mode);
            run_script(&mut fs, big);
            assert!(!fs.needs_recovery());
            assert_eq!(fs.journal_info().unwrap().start, 0);
            assert_tools_agree(&fs, &format!("ext3-sequence-{}", mode.as_str()));
        }
    }

    #[test]
    fn logdump_reads_our_last_transaction_tag_for_tag() {
        for mode in BOTH {
            let mut fs = ext3(mode);
            run_script(&mut fs, 200 * 1024);
            // A reload restarts the log at index 1 (spec section 4), so the
            // walk logdump makes from block 1 reaches the next transaction.
            let mut fs = ExtFs::from_image(fs.disk().as_bytes().to_vec()).unwrap();
            let tid = fs.journal_info().unwrap().sequence;
            let mut data = pattern(3000);
            data[..4].copy_from_slice(&[0xC0, 0x3B, 0x39, 0x98]);
            let rec = fs.create_file("/logged.bin", &data).unwrap();
            let want = logdump_lines(&rec, tid, true);
            let name = format!("ext3-logdump-{}", mode.as_str());
            let Some(got) = logdump(&fs, &name, "logdump -aO", tid) else {
                return;
            };
            assert_eq!(got, want, "{mode:?}");
            // Data mode tags the file's first block and escapes it.
            let escaped = want.iter().any(|l| l.ends_with("(flags 0x3)"));
            assert_eq!(escaped, mode == JournalMode::Data, "{want:?}");
        }
    }
}

/// ext3 Task 5: `e2fsck -fy` as the replay oracle (ext3 spec section 10.3,
/// third and fourth bullets): on a copy of a crashed image it must write
/// exactly the bytes `recover()` writes.
mod ext3_recovery {
    use super::common::{assert_images_agree, ext3, SUPERBLOCK_IGNORED};
    use super::ext3_transactions::{logdump, logdump_lines};
    use super::{both_streams, image_file, pattern, run, tool};
    use ext::{encode_commit, encode_descriptor, CrashPhase, ExtFs, JournalMode, Tag, TAG_ESCAPE};

    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    const PHASES: [CrashPhase; 3] = [
        CrashPhase::BeforeCommit,
        CrashPhase::AfterCommit,
        CrashPhase::DuringCheckpoint,
    ];
    const MAGIC: [u8; 4] = [0xC0, 0x3B, 0x39, 0x98];
    /// Run `e2fsck -fy` on a copy of `fs`'s image and `e2fsck -fn` after
    /// it, check both, and return the repaired bytes. `None` without
    /// e2fsck.
    fn e2fsck_recovered(fs: &ExtFs, name: &str) -> Option<Vec<u8>> {
        let e2fsck = tool("e2fsck")?;
        let image = image_file(fs, name);
        let fix = run(&e2fsck, &["-fy"], &image);
        let check = run(&e2fsck, &["-fn"], &image);
        let bytes = std::fs::read(&image).unwrap();
        std::fs::remove_file(&image).unwrap();
        let text = both_streams(&fix);
        assert!(
            matches!(fix.status.code(), Some(0 | 1)),
            "e2fsck -fy {name}:\n{text}"
        );
        assert!(text.contains("recovering journal"), "{name}:\n{text}");
        let text = both_streams(&check);
        assert_eq!(check.status.code(), Some(0), "e2fsck -fn {name}:\n{text}");
        assert!(!text.contains("Fix?"), "{name}:\n{text}");
        Some(bytes)
    }

    #[test]
    fn assert_images_agree_ignores_only_the_listed_superblock_fields() {
        let fs = ext3(JournalMode::Ordered);
        let ours = fs.disk().as_bytes().to_vec();
        let mut theirs = ours.clone();
        for (offset, len, _) in SUPERBLOCK_IGNORED {
            for sb in [1024, 8193 * 1024] {
                theirs[sb + offset..sb + offset + len].fill(0xEE);
            }
        }
        let geo = fs.geometry();
        assert_images_agree(&ours, &theirs, geo, "e2fsck's");
        for at in [1024 + 0x0C, 8193 * 1024 + 0x60, 82 * 1024 + 0x18, 69 * 1024] {
            let mut theirs = ours.clone();
            theirs[at] ^= 1;
            let message = std::panic::catch_unwind(|| {
                assert_images_agree(&ours, &theirs, geo, "e2fsck's");
            })
            .unwrap_err();
            let message = message.downcast_ref::<String>().unwrap();
            let want = format!("block {} differs at offset 0x{:03X}", at / 1024, at % 1024);
            assert!(message.starts_with(&want), "{message}");
        }
    }

    #[test]
    fn e2fsck_replays_a_crashed_create_to_the_bytes_recover_writes() {
        if tool("e2fsck").is_none() || tool("debugfs").is_none() {
            return;
        }
        for mode in BOTH {
            for phase in PHASES {
                let mut fs = ext3(mode);
                fs.create_dir("/d").unwrap();
                let tid = fs.journal_info().unwrap().sequence;
                fs.arm_crash(phase).unwrap();
                let rec = fs.create_file("/d/f", &pattern(3000)).unwrap();
                let name = format!("ext3-replay-{}-{}", mode.as_str(), phase.as_str());
                // logdump reads our descriptor, tags, and (once written) commit.
                let with_commit = phase != CrashPhase::BeforeCommit;
                let got = logdump(&fs, &name, "logdump -a", tid).unwrap();
                assert_eq!(got, logdump_lines(&rec, tid, with_commit), "{name}");

                let theirs = e2fsck_recovered(&fs, &name).unwrap();
                fs.recover().unwrap();
                assert_images_agree(fs.disk().as_bytes(), &theirs, fs.geometry(), "e2fsck's");
            }
        }
    }

    #[test]
    fn e2fsck_replays_an_escaped_copy_to_the_bytes_recover_writes() {
        if tool("e2fsck").is_none() {
            return;
        }
        for phase in [CrashPhase::AfterCommit, CrashPhase::DuringCheckpoint] {
            let mut fs = ext3(JournalMode::Data);
            let mut data = pattern(3000);
            data[..4].copy_from_slice(&MAGIC);
            fs.arm_crash(phase).unwrap();
            let rec = fs.create_file("/magic", &data).unwrap();
            assert!(
                rec.events
                    .iter()
                    .any(|e| e.to_string().ends_with("(block 91) (escaped)")),
                "the first data block's copy is escaped"
            );
            let name = format!("ext3-replay-escaped-{}", phase.as_str());
            let theirs = e2fsck_recovered(&fs, &name).unwrap();
            fs.recover().unwrap();
            assert_images_agree(fs.disk().as_bytes(), &theirs, fs.geometry(), "e2fsck's");
            assert_eq!(theirs[1111 * 1024..1111 * 1024 + 4], MAGIC);
            assert_eq!(fs.read_file("/magic").unwrap(), data);
        }
    }

    #[test]
    fn e2fsck_replays_a_foreign_log_of_two_transactions_to_the_bytes_recover_writes() {
        if tool("e2fsck").is_none() {
            return;
        }
        let fs = ext3(JournalMode::Ordered);
        let uuid = fs.superblock().uuid;
        let mut image = fs.disk().as_bytes().to_vec();
        let mut put = |index: usize, block: &[u8]| {
            let at = (82 + index) * 1024;
            image[at..at + 1024].copy_from_slice(block);
        };
        let tag = |block, flags| Tag { block, flags };
        let mut escaped = [0xC3; 1024];
        escaped[..4].fill(0);
        // Transaction 7 wraps from journal block 1021 to its commit at 1;
        // transaction 8 rewrites block 2001 and escapes 2002; transaction 9
        // never commits.
        put(
            1021,
            &encode_descriptor(7, &uuid, &[tag(2000, 0), tag(2001, 0)]),
        );
        put(1022, &[0xA1; 1024]);
        put(1023, &[0xB1; 1024]);
        put(1, &encode_commit(7, 1));
        put(
            2,
            &encode_descriptor(8, &uuid, &[tag(2001, 0), tag(2002, TAG_ESCAPE)]),
        );
        put(3, &[0xB2; 1024]);
        put(4, &escaped);
        put(5, &encode_commit(8, 1));
        put(6, &encode_descriptor(9, &uuid, &[tag(2003, 0)]));
        put(7, &[0xD1; 1024]);
        let jsb = 82 * 1024 + 0x18;
        image[jsb..jsb + 8].copy_from_slice(&[0, 0, 0, 7, 0, 0, 0x03, 0xFD]);
        image[1024 + 0x60] = 0x06;
        let mut fs = ExtFs::from_image(image).unwrap();
        let theirs = e2fsck_recovered(&fs, "ext3-replay-foreign").unwrap();
        fs.recover().unwrap();
        assert_images_agree(fs.disk().as_bytes(), &theirs, fs.geometry(), "e2fsck's");
        assert_eq!(theirs[jsb..jsb + 8], [0, 0, 0, 10, 0, 0, 0, 0]);
        assert_eq!(theirs[2002 * 1024..2002 * 1024 + 4], MAGIC);
    }
}
