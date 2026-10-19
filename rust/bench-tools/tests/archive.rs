use bench_tools::archive::{
    self, ArchiveError, Limits, NAME_RECORD_LIMIT, PackageWriter, Refusal, ScratchWriter,
    TarMember, ZipMember,
};
use bench_tools::sha256;
use std::cell::Cell;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::rc::Rc;

const ROOMY: Limits = Limits {
    members: 64,
    payload: 64 * 1024 * 1024,
};

fn header(name: &[u8], kind: u8, size: u64, mode: u32) -> Vec<u8> {
    let mut header = tar::Header::new_ustar();
    header.as_old_mut().name[..name.len()].copy_from_slice(name);
    header.set_entry_type(tar::EntryType::new(kind));
    header.set_size(size);
    header.set_mode(mode);
    header.set_cksum();
    header.as_bytes().to_vec()
}

fn padded(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    out.resize(data.len().div_ceil(512) * 512, 0);
    out
}

fn entry(name: &[u8], kind: u8, mode: u32, data: &[u8]) -> Vec<u8> {
    [header(name, kind, data.len() as u64, mode), padded(data)].concat()
}

fn pax(kind: u8, records: &[u8]) -> Vec<u8> {
    entry(b"././@PaxHeader", kind, 0o644, records)
}

fn pax_record(keyword: &str, value: &str) -> Vec<u8> {
    let base = keyword.len() + value.len() + 3;
    let mut length = 0;
    while base + length.to_string().len() != length {
        length = base + length.to_string().len();
    }
    format!("{length} {keyword}={value}\n").into_bytes()
}

fn tar_of(parts: &[Vec<u8>]) -> Vec<u8> {
    [parts.concat(), vec![0; 1024]].concat()
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut writer = archive::gzip_writer(Vec::new(), 6);
    writer.write_all(data).unwrap();
    writer.finish().unwrap()
}

type Read_ = Result<Vec<(TarMember, Vec<u8>)>, ArchiveError>;

fn read_tar(compressed: &[u8], limits: &Limits) -> Read_ {
    let mut members = Vec::new();
    archive::read_tar_gz(compressed, limits, |member, data| {
        let mut bytes = Vec::new();
        data.read_to_end(&mut bytes)?;
        members.push((member.clone(), bytes));
        Ok(())
    })?;
    Ok(members)
}

fn refusal(result: Read_) -> Refusal {
    match result {
        Err(ArchiveError::Refused(refusal)) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn io_error<T: std::fmt::Debug>(result: Result<T, ArchiveError>) -> io::Error {
    match result {
        Err(ArchiveError::Io(error)) => error,
        other => panic!("expected an I/O error, got {other:?}"),
    }
}

/// Counts the bytes a reader hands out.
struct Counting<R> {
    inner: R,
    read: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.read.set(self.read.get() + read as u64);
        Ok(read)
    }
}

impl<R: Seek> Seek for Counting<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}

#[test]
fn packages_round_trip_long_empty_and_non_utf8_names_with_python_gzip_headers() {
    let long = [b"bench/".as_slice(), &[b'n'; 150]].concat();
    let longer_non_utf8 = [b"docs/\xff".as_slice(), &[b'x'; 300]].concat();
    let members: Vec<(Vec<u8>, u32, Vec<u8>)> = vec![
        (b"bench/a.txt".to_vec(), 0o644, b"alpha\n".to_vec()),
        (long, 0o755, vec![7; 1000]),
        (b"bench/empty".to_vec(), 0o600, Vec::new()),
        (b"artifacts/\xfe\xff.bin".to_vec(), 0o000, vec![0xff; 513]),
        (longer_non_utf8, 0o640, b"z".to_vec()),
    ];
    let mut writer = PackageWriter::new(Vec::new());
    for (name, mode, data) in &members {
        writer
            .append(name, data.len() as u64, *mode, &mut data.as_slice())
            .unwrap();
    }
    let compressed = writer.finish().unwrap();
    assert_eq!(
        &compressed[..10],
        b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\xff"
    );
    let read = read_tar(&compressed, &ROOMY).unwrap();
    let expected: Vec<(TarMember, Vec<u8>)> = members
        .into_iter()
        .map(|(name, mode, data)| {
            let size = data.len() as u64;
            (TarMember { name, size, mode }, data)
        })
        .collect();
    assert_eq!(read, expected);
}

#[test]
fn a_short_member_source_is_refused_while_writing() {
    let mut writer = PackageWriter::new(Vec::new());
    let error = io_error(writer.append(b"bench/a", 10, 0o644, &mut b"short".as_slice()));
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert!(matches!(
        writer.append(b"bench/b", 0, 0o4644, &mut io::empty()),
        Err(ArchiveError::Refused(Refusal::SpecialMode { .. }))
    ));
}

#[test]
fn gnu_long_names_and_pax_paths_name_the_next_member_and_the_first_one_applies() {
    let name = [b"bench/".as_slice(), &[b'g'; 200]].concat();
    let gnu = tar_of(&[
        entry(
            b"././@LongLink",
            b'L',
            0o644,
            &[name.as_slice(), b"\0"].concat(),
        ),
        entry(b"short", b'0', 0o644, b"data"),
    ]);
    let read = read_tar(&gzip(&gnu), &ROOMY).unwrap();
    assert_eq!(read[0].0.name, name);
    let both = tar_of(&[
        pax(b'x', b"20 path=bench/first\n"),
        entry(b"././@LongLink", b'L', 0o644, b"bench/second\0"),
        entry(b"short", b'0', 0o644, b"data"),
    ]);
    assert_eq!(
        read_tar(&gzip(&both), &ROOMY).unwrap()[0].0.name,
        b"bench/first"
    );
    let prefixed = {
        let mut block = header(b"name", b'0', 1, 0o644);
        block[345..351].copy_from_slice(b"bench/");
        let mut header = tar::Header::new_old();
        header.as_mut_bytes().copy_from_slice(&block);
        header.set_cksum();
        tar_of(&[[header.as_bytes().to_vec(), padded(b"x")].concat()])
    };
    assert_eq!(
        read_tar(&gzip(&prefixed), &ROOMY).unwrap()[0].0.name,
        b"bench//name"
    );
}

#[test]
fn multi_member_gzip_with_zero_padding_reads_as_python_gzip_does() {
    let tar = tar_of(&[entry(b"bench/a", b'0', 0o644, &[5; 3000])]);
    let (first, second) = tar.split_at(1000);
    let compressed = [gzip(first), vec![0; 7], gzip(second), vec![0; 3]].concat();
    let read = read_tar(&compressed, &ROOMY).unwrap();
    assert_eq!(read[0].1, vec![5; 3000]);
}

#[test]
fn entries_after_the_end_marker_are_never_parsed() {
    let symlink = entry(b"bench/link", b'2', 0o777, b"");
    let tar = [tar_of(&[entry(b"bench/a", b'0', 0o644, b"a")]), symlink].concat();
    assert_eq!(read_tar(&gzip(&tar), &ROOMY).unwrap().len(), 1);
}

#[test]
fn a_bad_crc_or_truncated_stream_is_an_error() {
    let compressed = gzip(&tar_of(&[entry(b"bench/a", b'0', 0o644, b"abc")]));
    let mut bad_crc = compressed.clone();
    let at = bad_crc.len() - 8;
    bad_crc[at] ^= 0xff;
    io_error(read_tar(&bad_crc, &ROOMY));
    io_error(read_tar(&compressed[..compressed.len() / 2], &ROOMY));
    let mut cut = entry(b"bench/a", b'0', 0o644, &[1; 2000]);
    cut.truncate(1024);
    let error = io_error(read_tar(&gzip(&cut), &ROOMY));
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn a_pax_size_record_is_refused_however_large() {
    let tar = tar_of(&[
        pax(b'x', b"29 size=99999999999999999999\n"),
        entry(b"bench/a", b'0', 0o644, b"a"),
    ]);
    assert_eq!(
        refusal(read_tar(&gzip(&tar), &ROOMY)),
        Refusal::PaxSize {
            name: b"bench/a".to_vec()
        }
    );
}

/// A 2 MiB name record is refused from its header: the reader stops long before the record's bytes.
#[test]
fn two_mebibyte_pax_and_gnu_name_records_are_refused_before_they_are_read() {
    let huge = vec![b'n'; 2 * 1024 * 1024];
    for kind in *b"xL" {
        let body = if kind == b'x' {
            [b"2097164 path=".as_slice(), &huge[..2_097_164 - 14], b"\n"].concat()
        } else {
            huge.clone()
        };
        let tar = tar_of(&[pax(kind, &body), entry(b"bench/a", b'0', 0o644, b"a")]);
        let mut writer = archive::gzip_writer(Vec::new(), 0);
        writer.write_all(&tar).unwrap();
        let stored = writer.finish().unwrap();
        let read = Rc::new(Cell::new(0));
        let source = Counting {
            inner: stored.as_slice(),
            read: read.clone(),
        };
        let result = archive::read_tar_gz(source, &ROOMY, |_, _| Ok(()));
        assert!(matches!(
            result,
            Err(ArchiveError::Refused(Refusal::NameRecord { size })) if size == body.len() as u64
        ));
        assert!(read.get() < NAME_RECORD_LIMIT, "read {} bytes", read.get());
    }
}

#[test]
fn links_devices_directories_and_fifos_are_refused_by_type() {
    for (kind, label) in [
        (b'1', "hard link"),
        (b'2', "symbolic link"),
        (b'3', "character device"),
        (b'4', "block device"),
        (b'5', "directory"),
        (b'6', "fifo"),
        (b'V', "unsupported type"),
    ] {
        let tar = tar_of(&[entry(b"bench/x", kind, 0o644, b"")]);
        assert_eq!(
            refusal(read_tar(&gzip(&tar), &ROOMY)),
            Refusal::NotRegular {
                name: b"bench/x".to_vec(),
                kind: label.to_string()
            }
        );
    }
    let mut old = tar::Header::new_ustar();
    old.as_old_mut().name[..10].copy_from_slice(b"bench/dir/");
    old.set_mode(0o755);
    old.set_size(0);
    old.as_mut_bytes()[156] = 0;
    old.set_cksum();
    let old_directory = tar_of(&[old.as_bytes().to_vec()]);
    assert!(matches!(
        refusal(read_tar(&gzip(&old_directory), &ROOMY)),
        Refusal::NotRegular { kind, .. } if kind == "directory"
    ));
    let contiguous = tar_of(&[entry(b"bench/c", b'7', 0o644, b"c")]);
    assert_eq!(read_tar(&gzip(&contiguous), &ROOMY).unwrap()[0].1, b"c");
}

#[test]
fn a_regular_member_with_special_mode_bits_is_refused() {
    for mode in [0o4755, 0o2755, 0o1777, 0o100_644, 0o120_777] {
        let tar = tar_of(&[entry(b"bench/x", b'0', mode, b"x")]);
        assert_eq!(
            refusal(read_tar(&gzip(&tar), &ROOMY)),
            Refusal::SpecialMode {
                name: b"bench/x".to_vec(),
                mode
            }
        );
    }
}

#[test]
fn duplicate_global_sparse_and_dangling_headers_are_refused() {
    let duplicate = tar_of(&[
        entry(b"bench/a", b'0', 0o644, b"1"),
        entry(b"bench/a", b'0', 0o644, b"2"),
    ]);
    assert_eq!(
        refusal(read_tar(&gzip(&duplicate), &ROOMY)),
        Refusal::Duplicate {
            name: b"bench/a".to_vec()
        }
    );
    let global = tar_of(&[
        pax(b'g', &pax_record("comment", "hey")),
        entry(b"bench/a", b'0', 0o644, b"1"),
    ]);
    assert_eq!(
        refusal(read_tar(&gzip(&global), &ROOMY)),
        Refusal::GlobalHeader
    );
    let gnu_sparse = tar_of(&[entry(b"bench/s", b'S', 0o644, b"")]);
    assert!(matches!(
        refusal(read_tar(&gzip(&gnu_sparse), &ROOMY)),
        Refusal::Sparse { .. }
    ));
    let pax_sparse = tar_of(&[
        pax(b'x', b"22 GNU.sparse.major=1\n"),
        entry(b"bench/s", b'0', 0o644, b""),
    ]);
    assert!(matches!(
        refusal(read_tar(&gzip(&pax_sparse), &ROOMY)),
        Refusal::Sparse { .. }
    ));
    let dangling = tar_of(&[pax(b'x', b"20 path=bench/first\n")]);
    assert!(matches!(
        refusal(read_tar(&gzip(&dangling), &ROOMY)),
        Refusal::Malformed(_)
    ));
    let framing = tar_of(&[
        pax(b'x', b"99 path=bench/a\n"),
        entry(b"bench/a", b'0', 0o644, b""),
    ]);
    assert!(matches!(
        refusal(read_tar(&gzip(&framing), &ROOMY)),
        Refusal::Malformed(_)
    ));
}

#[test]
fn an_invalid_header_after_the_last_member_is_an_error() {
    let mut garbage = header(b"bench/b", b'0', 0, 0o644);
    garbage[148] ^= 1;
    let tar = tar_of(&[entry(b"bench/a", b'0', 0o644, b"a"), garbage]);
    io_error(read_tar(&gzip(&tar), &ROOMY));
}

#[test]
fn the_index_limits_bytes_headers_and_members() {
    let tar = tar_of(&[
        entry(b"bench/a", b'0', 0o644, b"a"),
        entry(b"bench/b", b'0', 0o644, b"b"),
    ]);
    let one = Limits {
        members: 1,
        payload: 2,
    };
    assert_eq!(
        refusal(read_tar(&gzip(&tar), &one)),
        Refusal::TooManyEntries { limit: 1 }
    );
    let big = tar_of(&[entry(b"bench/a", b'0', 0o644, &vec![0; 4 * 1024 * 1024])]);
    let small = Limits {
        members: 1,
        payload: 1024,
    };
    assert_eq!(
        refusal(read_tar(&gzip(&big), &small)),
        Refusal::TooManyBytes {
            limit: small.tar_bytes()
        }
    );
    let headers = tar_of(&[
        pax(b'x', &pax_record("comment", "one")),
        pax(b'x', &pax_record("comment", "two")),
        pax(b'x', &pax_record("comment", "three")),
        pax(b'x', &pax_record("comment", "four")),
        entry(b"bench/a", b'0', 0o644, b"a"),
    ]);
    assert_eq!(
        refusal(read_tar(&gzip(&headers), &one)),
        Refusal::TooManyEntries { limit: 4 }
    );
}

fn scratch(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ScratchWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer.append(name, &mut &data[..]).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

type ZipRead = Result<Vec<(ZipMember, Vec<u8>)>, ArchiveError>;

fn read_zip(bytes: &[u8], limits: &Limits) -> ZipRead {
    let mut members = Vec::new();
    archive::read_zip(Cursor::new(bytes), limits, |member, data| {
        let mut out = Vec::new();
        data.read_to_end(&mut out)?;
        members.push((member.clone(), out));
        Ok(())
    })?;
    Ok(members)
}

fn zip_refusal(result: ZipRead) -> Refusal {
    match result {
        Err(ArchiveError::Refused(refusal)) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn positions(bytes: &[u8], signature: &[u8; 4]) -> Vec<usize> {
    bytes
        .windows(4)
        .enumerate()
        .filter(|(_, window)| window == signature)
        .map(|(at, _)| at)
        .collect()
}

/// Patch a field in every local header (offset `local`) and central header (offset `central`).
fn patch(bytes: &mut [u8], local: usize, central: usize, value: &[u8]) {
    for at in positions(bytes, b"PK\x03\x04") {
        bytes[at + local..at + local + value.len()].copy_from_slice(value);
    }
    for at in positions(bytes, b"PK\x01\x02") {
        bytes[at + central..at + central + value.len()].copy_from_slice(value);
    }
}

#[test]
fn scratch_archives_round_trip_with_python_entry_metadata() {
    let data = b"probe\n".repeat(100);
    let name = sha256::hex(&sha256::digest(&data));
    let bytes = scratch(&[(name.as_str(), &data), ("e", b"")]);
    let read = read_zip(&bytes, &ROOMY).unwrap();
    assert_eq!(
        read[0],
        (
            ZipMember {
                name: name.clone().into_bytes(),
                size: 600
            },
            data
        )
    );
    assert_eq!(read[1].0.size, 0);
    let central = positions(&bytes, b"PK\x01\x02")[0];
    assert_eq!(&bytes[central + 4..central + 6], &[20, 3], "made by Unix");
    assert_eq!(
        &bytes[central + 8..central + 16],
        &[0, 0, 8, 0, 0, 0, 0x21, 0],
        "flags, deflate, 1980-01-01 00:00"
    );
    assert_eq!(
        &bytes[central + 38..central + 42],
        &(0o600u32 << 16).to_le_bytes()
    );
    let mut duplicate = ScratchWriter::new(Cursor::new(Vec::new()));
    duplicate.append("a", &mut &b""[..]).unwrap();
    assert!(matches!(
        duplicate.append("a", &mut &b""[..]),
        Err(ArchiveError::Refused(Refusal::Duplicate { .. }))
    ));
}

#[test]
fn the_tracked_native_scratch_archives_hold_entries_named_by_their_hash() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for attempt in [
        "bench/runs/2026-10-10-last-push-claude-ce-opus/attempts/att-016",
        "bench/runs/2026-10-10-last-push-codex-thermo-sol61/attempts/att-007",
    ] {
        let directory = root.join(attempt);
        let index = std::fs::read_to_string(directory.join("native-artifacts.json")).unwrap();
        let index =
            bench_tools::pyjson::loads(&index, bench_tools::pyjson::Duplicates::KeepLast).unwrap();
        let rows = index
            .as_object()
            .unwrap()
            .get("files")
            .unwrap()
            .as_array()
            .unwrap()
            .len() as u64;
        let limits = Limits {
            members: rows,
            payload: 0,
        };
        let file = std::fs::File::open(directory.join("native-scratch.v1.zip")).unwrap();
        let mut entries = 0;
        archive::read_zip(file, &limits, |member, data| {
            let mut hasher = sha256::Sha256::new();
            let mut buf = vec![0; 8192];
            loop {
                let read = data.read(&mut buf)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buf[..read]);
            }
            assert_eq!(
                sha256::hex(&hasher.finalize()).as_bytes(),
                member.name.as_slice()
            );
            entries += 1;
            Ok(())
        })
        .unwrap();
        assert!(entries > 0, "{attempt}");
    }
}

/// Rewrite the end of `bytes` with zip64 end records claiming `entries`.
fn with_zip64_end(bytes: &[u8], entries: u64) -> Vec<u8> {
    with_zip64_end_extended(bytes, entries, 0)
}

/// As `with_zip64_end`, with `extensible` bytes of zip64 extensible data.
fn with_zip64_end_extended(bytes: &[u8], entries: u64, extensible: usize) -> Vec<u8> {
    let end = *positions(bytes, b"PK\x05\x06").last().unwrap();
    let directory_bytes = u64::from(u32::from_le_bytes(
        bytes[end + 12..end + 16].try_into().unwrap(),
    ));
    let directory_start = u64::from(u32::from_le_bytes(
        bytes[end + 16..end + 20].try_into().unwrap(),
    ));
    let mut out = bytes[..end].to_vec();
    let record_at = out.len() as u64;
    out.extend_from_slice(b"PK\x06\x06");
    out.extend_from_slice(&(44 + extensible as u64).to_le_bytes());
    out.extend_from_slice(&[45, 3, 45, 0]);
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&entries.to_le_bytes());
    out.extend_from_slice(&entries.to_le_bytes());
    out.extend_from_slice(&directory_bytes.to_le_bytes());
    out.extend_from_slice(&directory_start.to_le_bytes());
    out.extend_from_slice(&vec![b'x'; extensible]);
    out.extend_from_slice(b"PK\x06\x07");
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&record_at.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(
        b"PK\x05\x06\0\0\0\0\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\0\0",
    );
    out
}

#[test]
fn zip64_end_records_are_read_and_a_million_entry_claim_is_refused_before_the_directory() {
    let bytes = scratch(&[("a", b"alpha"), ("b", b"beta")]);
    let zip64 = with_zip64_end(&bytes, 2);
    let read = read_zip(&zip64, &ROOMY).unwrap();
    assert_eq!(read.len(), 2);
    let million = with_zip64_end(&bytes, 1_000_000);
    let read = Rc::new(Cell::new(0));
    let source = Counting {
        inner: Cursor::new(million.clone()),
        read: read.clone(),
    };
    let result = archive::read_zip(source, &ROOMY, |_, _| Ok(()));
    assert!(matches!(
        result,
        Err(ArchiveError::Refused(Refusal::ZipDirectory {
            entries: 1_000_000,
            ..
        }))
    ));
    assert!(
        read.get() < 4 * million.len() as u64,
        "read {} bytes",
        read.get()
    );
    let limits = Limits {
        members: 1,
        payload: 0,
    };
    assert!(matches!(
        zip_refusal(read_zip(&bytes, &limits)),
        Refusal::ZipDirectory { entries: 2, .. }
    ));
}

#[test]
fn duplicate_encrypted_unsupported_and_non_regular_zip_entries_are_refused() {
    let mut duplicate = scratch(&[("aa", b"1"), ("bb", b"2")]);
    for at in positions(&duplicate, b"PK\x03\x04").into_iter().skip(1) {
        duplicate[at + 30..at + 32].copy_from_slice(b"aa");
    }
    for at in positions(&duplicate, b"PK\x01\x02").into_iter().skip(1) {
        duplicate[at + 46..at + 48].copy_from_slice(b"aa");
    }
    assert_eq!(
        zip_refusal(read_zip(&duplicate, &ROOMY)),
        Refusal::DuplicateZipEntries
    );

    let mut encrypted = scratch(&[("a", b"secret")]);
    patch(&mut encrypted, 6, 8, &[1, 0]);
    assert!(matches!(
        zip_refusal(read_zip(&encrypted, &ROOMY)),
        Refusal::Encrypted { .. }
    ));

    let mut bzip2 = scratch(&[("a", b"data")]);
    patch(&mut bzip2, 8, 10, &[12, 0]);
    assert!(matches!(
        zip_refusal(read_zip(&bzip2, &ROOMY)),
        Refusal::Method { method, .. } if method.contains("12") || method.to_lowercase().contains("bzip")
    ));

    let mut symlink = scratch(&[("a", b"target")]);
    for at in positions(&symlink, b"PK\x01\x02") {
        symlink[at + 38..at + 42].copy_from_slice(&(0o120_777u32 << 16).to_le_bytes());
    }
    assert!(matches!(
        zip_refusal(read_zip(&symlink, &ROOMY)),
        Refusal::NotRegular { kind, .. } if kind == "symbolic link"
    ));

    let directory = {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_directory("dir/", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap().into_inner()
    };
    assert!(matches!(
        zip_refusal(read_zip(&directory, &ROOMY)),
        Refusal::NotRegular { kind, .. } if kind == "directory"
    ));
}

#[test]
fn a_bad_zip_crc_is_an_error() {
    let mut bytes = scratch(&[("a", b"payload")]);
    patch(&mut bytes, 14, 16, &[0, 0, 0, 0]);
    io_error(read_zip(&bytes, &ROOMY));
}

#[test]
fn a_pax_header_uses_its_last_path_record_without_trailing_slashes_as_tarfile_does() {
    let mut records = pax_record("path", "bench/first");
    records.extend(pax_record("path", "bench/second//"));
    let tar = tar_of(&[pax(b'x', &records), entry(b"short", b'0', 0o644, b"data")]);
    assert_eq!(
        read_tar(&gzip(&tar), &ROOMY).unwrap()[0].0.name,
        b"bench/second"
    );
}

#[test]
fn zip64_extensible_data_over_the_limit_is_refused_before_it_is_read() {
    let bytes = scratch(&[("a", b"alpha")]);
    let small = with_zip64_end_extended(&bytes, 1, 16);
    assert_eq!(read_zip(&small, &ROOMY).unwrap().len(), 1);
    let large = with_zip64_end_extended(&bytes, 1, 2 * 1024 * 1024);
    let read = Rc::new(Cell::new(0));
    let source = Counting {
        inner: Cursor::new(large),
        read: read.clone(),
    };
    let result = archive::read_zip(source, &ROOMY, |_, _| Ok(()));
    assert!(matches!(
        result,
        Err(ArchiveError::Refused(Refusal::Zip64Extensible { bytes })) if bytes == 2 * 1024 * 1024
    ));
    assert!(read.get() < 256 * 1024, "read {} bytes", read.get());
}

#[test]
fn every_zip64_end_record_after_the_locator_is_bounded_not_only_the_first() {
    let bytes = scratch(&[("a", b"alpha")]);
    let large = with_zip64_end_extended(&bytes, 1, 2 * 1024 * 1024);
    let record_at = positions(&large, b"PK\x06\x06")[0];
    let mut decoy = b"PK\x06\x06".to_vec();
    decoy.extend_from_slice(&44u64.to_le_bytes());
    decoy.resize(56, 0);
    let with_decoy = [&large[..record_at], &decoy, &large[record_at..]].concat();
    assert!(matches!(
        zip_refusal(read_zip(&with_decoy, &ROOMY)),
        Refusal::Zip64Extensible { bytes } if bytes == 2 * 1024 * 1024
    ));
}

#[test]
fn payload_signatures_are_not_scanned_before_the_first_member_and_prepended_data_still_reads() {
    let payload = b"PK\x01\x02".repeat(256 * 1024);
    let bytes = {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("a", options).unwrap();
        writer.write_all(&payload).unwrap();
        writer.finish().unwrap().into_inner()
    };
    let limits = Limits {
        members: 1,
        payload: payload.len() as u64,
    };
    let read = Rc::new(Cell::new(0));
    let source = Counting {
        inner: Cursor::new(bytes.clone()),
        read: read.clone(),
    };
    let mut before_first = None;
    archive::read_zip(source, &limits, |_, _| {
        before_first.get_or_insert(read.get());
        Ok(())
    })
    .unwrap();
    let before_first = before_first.unwrap();
    assert!(
        before_first < 4 * limits.zip_directory_bytes(),
        "read {before_first} bytes before the first member"
    );

    let prepended = [&b"#!/bin/sh\n".repeat(100)[..], &bytes].concat();
    assert_eq!(read_zip(&prepended, &limits).unwrap()[0].1, payload);
    let zip64 = with_zip64_end(&scratch(&[("a", b"alpha")]), 1);
    let prepended64 = [&b"#!/bin/sh\n".repeat(100)[..], &zip64].concat();
    assert_eq!(read_zip(&prepended64, &ROOMY).unwrap()[0].1, b"alpha");
}

#[test]
fn indexed_member_sizes_that_overflow_are_an_error_not_a_panic() {
    let directory = std::env::temp_dir().join(format!(
        "bench-tools-archive-overflow-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let mut writer = PackageWriter::new(Vec::new());
    writer.append(b"bench/a", 1, 0o644, &mut &b"a"[..]).unwrap();
    let compressed = writer.finish().unwrap();
    let sha = sha256::hex_digest(&compressed);
    std::fs::write(directory.join(format!("{sha}.tar.gz")), &compressed).unwrap();
    let member = |path: &str, bytes| archive::IndexedMember {
        path: path.to_string(),
        sha256: sha256::hex_digest(b"a"),
        bytes,
        mode: 0o644,
    };
    let package = archive::IndexedPackage {
        asset: format!("{sha}.tar.gz"),
        sha256: sha,
        bytes: compressed.len() as u64,
        members: vec![member("bench/a", u64::MAX), member("bench/b", 1)],
    };
    assert_eq!(
        archive::check_package(&directory, &package),
        Err("indexed member sizes overflow".to_string())
    );
    std::fs::remove_dir_all(directory).unwrap();
}
