//! Gzip, tar and zip reading and writing over the `flate2`, `tar` and `zip` crates, with this repository's metadata
//! limits and refusals. Readers stream each regular member to a visitor; writers stream each member in.
//!
//! The readers accept what Python's `gzip`, `tarfile` and `zipfile` accept for evidence packages and native scratch
//! archives, except the refusals `archive-decisions.v1.json` records.

use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use flate2::{Compression, GzBuilder};
use std::collections::HashSet;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, System, ZipArchive, ZipWriter};

const BLOCK: u64 = 512;
/// The largest PAX record set or GNU long name a member may carry.
pub const NAME_RECORD_LIMIT: u64 = 64 * 1024;
/// A member may be preceded by a PAX header, a GNU long name and a GNU long link.
const EXTENSIONS_PER_MEMBER: u64 = 3;
/// `tarfile` pads an archive to a multiple of its 10240-byte record.
const END_ALLOWANCE: u64 = 20 * BLOCK;
const ZIP_END: u64 = 22;
const ZIP_COMMENT_LIMIT: u64 = 65_535;
const ZIP_CENTRAL_ENTRY: u64 = 46;
const ZIP_END_SIGNATURE: [u8; 4] = *b"PK\x05\x06";
const ZIP64_LOCATOR_SIGNATURE: [u8; 4] = *b"PK\x06\x07";
const ZIP64_END_SIGNATURE: [u8; 4] = *b"PK\x06\x06";

/// What an archive's index allows: its member count and the sum of their sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub members: u64,
    pub payload: u64,
}

impl Limits {
    /// Decoded tar bytes: the payload, each member's headers, padding and capped name records, and the end padding.
    pub fn tar_bytes(&self) -> u64 {
        let record = BLOCK + NAME_RECORD_LIMIT + BLOCK - 1;
        let per_member = BLOCK + BLOCK - 1 + EXTENSIONS_PER_MEMBER * record;
        self.payload
            .saturating_add(self.members.saturating_mul(per_member))
            .saturating_add(END_ALLOWANCE)
    }

    pub fn tar_headers(&self) -> u64 {
        self.members.saturating_mul(1 + EXTENSIONS_PER_MEMBER)
    }

    /// Central directory bytes: a fixed entry with up to `NAME_RECORD_LIMIT` of name, extra field and comment.
    pub fn zip_directory_bytes(&self) -> u64 {
        self.members
            .saturating_mul(ZIP_CENTRAL_ENTRY + NAME_RECORD_LIMIT)
    }
}

/// Why an archive is refused, as opposed to failing to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotRegular { name: Vec<u8>, kind: String },
    SpecialMode { name: Vec<u8>, mode: u32 },
    Duplicate { name: Vec<u8> },
    DuplicateZipEntries,
    Encrypted { name: Vec<u8> },
    Method { name: Vec<u8>, method: String },
    GlobalHeader,
    Sparse { name: Vec<u8> },
    PaxSize { name: Vec<u8> },
    NameRecord { size: u64 },
    TooManyEntries { limit: u64 },
    TooManyBytes { limit: u64 },
    ZipDirectory { entries: u64, bytes: u64 },
    Malformed(String),
}

fn shown(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotRegular { name, kind } => {
                write!(
                    f,
                    "archive member is not a regular file ({kind}): {}",
                    shown(name)
                )
            }
            Refusal::SpecialMode { name, mode } => {
                write!(
                    f,
                    "archive member has special mode bits {mode:o}: {}",
                    shown(name)
                )
            }
            Refusal::Duplicate { name } => write!(f, "duplicate archive member: {}", shown(name)),
            Refusal::DuplicateZipEntries => {
                f.write_str("zip central directory names a member twice")
            }
            Refusal::Encrypted { name } => {
                write!(f, "archive member is encrypted: {}", shown(name))
            }
            Refusal::Method { name, method } => write!(
                f,
                "archive member uses compression method {method}, not stored or deflate: {}",
                shown(name)
            ),
            Refusal::GlobalHeader => f.write_str("archive has a PAX global header"),
            Refusal::Sparse { name } => write!(f, "archive member is sparse: {}", shown(name)),
            Refusal::PaxSize { name } => {
                write!(f, "archive member has a PAX size record: {}", shown(name))
            }
            Refusal::NameRecord { size } => write!(
                f,
                "archive extension header of {size} bytes exceeds {NAME_RECORD_LIMIT} bytes"
            ),
            Refusal::TooManyEntries { limit } => {
                write!(f, "archive has more than {limit} entries")
            }
            Refusal::TooManyBytes { limit } => {
                write!(f, "archive decodes to more than {limit} bytes")
            }
            Refusal::ZipDirectory { entries, bytes } => write!(
                f,
                "zip central directory claims {entries} entries in {bytes} bytes, above its limit"
            ),
            Refusal::Malformed(message) => write!(f, "malformed archive: {message}"),
        }
    }
}

impl std::error::Error for Refusal {}

#[derive(Debug)]
pub enum ArchiveError {
    /// Reading, decompressing or writing failed, including a bad checksum or a truncated stream.
    Io(io::Error),
    Refused(Refusal),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::Io(error) => write!(f, "{error}"),
            ArchiveError::Refused(refusal) => write!(f, "{refusal}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<io::Error> for ArchiveError {
    /// A refusal raised inside a crate's read comes back wrapped in an `io::Error`.
    fn from(error: io::Error) -> Self {
        match error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<Refusal>())
        {
            Some(refusal) => ArchiveError::Refused(refusal.clone()),
            None => ArchiveError::Io(error),
        }
    }
}

impl From<Refusal> for ArchiveError {
    fn from(refusal: Refusal) -> Self {
        ArchiveError::Refused(refusal)
    }
}

impl From<zip::result::ZipError> for ArchiveError {
    fn from(error: zip::result::ZipError) -> Self {
        match error {
            zip::result::ZipError::Io(error) => error.into(),
            other => ArchiveError::Refused(Refusal::Malformed(other.to_string())),
        }
    }
}

fn refused(refusal: Refusal) -> io::Error {
    io::Error::other(refusal)
}

/// Reads at most `limit` bytes and refuses a stream that holds more.
struct Budget<R> {
    inner: R,
    remaining: u64,
    limit: u64,
}

impl<R: Read> Read for Budget<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(refused(Refusal::TooManyBytes { limit: self.limit })),
            };
        }
        let wanted = buf
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buf[..wanted])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

/// Exactly `remaining` bytes of `inner`: a shorter stream is an error, not a short member.
struct Exact<R> {
    inner: R,
    remaining: u64,
}

impl<R: Read> Read for Exact<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        let wanted = buf
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buf[..wanted])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("archive member ends {} bytes early", self.remaining),
            ));
        }
        self.remaining -= read as u64;
        Ok(read)
    }
}

/// Every gzip member in turn, as Python's `gzip` reads them: zero bytes between members are padding.
pub struct GzipReader<R: BufRead> {
    member: Option<GzDecoder<R>>,
}

impl<R: BufRead> GzipReader<R> {
    pub fn new(compressed: R) -> Self {
        Self {
            member: Some(GzDecoder::new(compressed)),
        }
    }
}

impl<R: BufRead> Read for GzipReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let Some(member) = self.member.as_mut() else {
                return Ok(0);
            };
            let read = member.read(buf)?;
            if read > 0 || buf.is_empty() {
                return Ok(read);
            }
            let Some(member) = self.member.take() else {
                return Ok(0);
            };
            let mut rest = member.into_inner();
            loop {
                let available = rest.fill_buf()?;
                if available.is_empty() {
                    return Ok(0);
                }
                let zeros = available.iter().take_while(|byte| **byte == 0).count();
                let only_zeros = zeros == available.len();
                rest.consume(zeros);
                if !only_zeros {
                    break;
                }
            }
            self.member = Some(GzDecoder::new(rest));
        }
    }
}

/// A gzip stream's decoded bytes, refused past `limit`.
pub fn gzip_reader<R: Read>(compressed: R, limit: u64) -> impl Read {
    Budget {
        inner: GzipReader::new(BufReader::new(compressed)),
        remaining: limit,
        limit,
    }
}

/// A gzip writer with no file name and modification time 0, as `GzipFile(filename='', mtime=0)` writes.
pub fn gzip_writer<W: Write>(out: W, level: u32) -> GzEncoder<W> {
    GzBuilder::new()
        .mtime(0)
        .write(out, Compression::new(level))
}

/// A regular member of a tar archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TarMember {
    pub name: Vec<u8>,
    pub size: u64,
    pub mode: u32,
}

/// The extension headers read since the last member. The first name applies, as in `tarfile`, which
/// applies each extension after the headers that follow it.
#[derive(Default)]
struct Pending {
    headers: u64,
    name: Option<Vec<u8>>,
    pax_size: bool,
    sparse: bool,
}

fn record(entry: &mut impl Read, size: u64) -> Result<Vec<u8>, ArchiveError> {
    if size > NAME_RECORD_LIMIT {
        return Err(Refusal::NameRecord { size }.into());
    }
    let mut data = Vec::with_capacity(size as usize);
    Exact {
        inner: entry,
        remaining: size,
    }
    .read_to_end(&mut data)?;
    Ok(data)
}

fn until_nul(bytes: &[u8]) -> &[u8] {
    bytes
        .iter()
        .position(|byte| *byte == 0)
        .map_or(bytes, |end| &bytes[..end])
}

/// PAX records as `tarfile._proc_pax` frames them: `<decimal length> <keyword>=<value>\n`, until a NUL.
/// A PAX keyword and its value.
type PaxRecord<'a> = (&'a [u8], &'a [u8]);

fn pax_records(buf: &[u8]) -> Result<Vec<PaxRecord<'_>>, Refusal> {
    let invalid = || Refusal::Malformed("invalid PAX header".to_string());
    let mut records = Vec::new();
    let mut pos = 0;
    while pos < buf.len() && buf[pos] != 0 {
        let digits = buf[pos..]
            .iter()
            .take(21)
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if !(1..=20).contains(&digits) || buf.get(pos + digits) != Some(&b' ') {
            return Err(invalid());
        }
        let length: usize = std::str::from_utf8(&buf[pos..pos + digits])
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or_else(invalid)?;
        let end = pos.checked_add(length).ok_or_else(invalid)?;
        if length < 5 || end > buf.len() || buf[end - 1] != b'\n' || pos + digits + 1 > end - 1 {
            return Err(invalid());
        }
        let field = &buf[pos + digits + 1..end - 1];
        let equals = field
            .iter()
            .position(|byte| *byte == b'=')
            .ok_or_else(invalid)?;
        if equals == 0 {
            return Err(invalid());
        }
        records.push((&field[..equals], &field[equals + 1..]));
        pos = end;
    }
    Ok(records)
}

/// The name `tarfile` reads from a header: `prefix/name` unless the type is a GNU one.
fn header_name(header: &[u8; 512]) -> Vec<u8> {
    let name = until_nul(&header[..100]);
    let prefix = until_nul(&header[345..500]);
    if prefix.is_empty() || matches!(header[156], b'L' | b'K' | b'S') {
        return name.to_vec();
    }
    [prefix, b"/", name].concat()
}

fn member_kind(kind: u8) -> &'static str {
    match kind {
        b'1' => "hard link",
        b'2' => "symbolic link",
        b'3' => "character device",
        b'4' => "block device",
        b'5' => "directory",
        b'6' => "fifo",
        _ => "unsupported type",
    }
}

/// Read a gzip-compressed tar archive, giving each regular member and its data to `visit` in archive order.
///
/// No entry after the end marker is parsed. The rest of the decoded stream is read through the byte limit, so
/// every gzip member's CRC and length are checked.
pub fn read_tar_gz<R: Read>(
    compressed: R,
    limits: &Limits,
    visit: impl FnMut(&TarMember, &mut dyn Read) -> Result<(), ArchiveError>,
) -> Result<(), ArchiveError> {
    let decoded = Budget {
        inner: GzipReader::new(BufReader::new(compressed)),
        remaining: limits.tar_bytes(),
        limit: limits.tar_bytes(),
    };
    let mut rest = read_tar(decoded, limits, visit)?;
    io::copy(&mut rest, &mut io::sink())?;
    Ok(())
}

fn read_tar<R: Read>(
    decoded: R,
    limits: &Limits,
    mut visit: impl FnMut(&TarMember, &mut dyn Read) -> Result<(), ArchiveError>,
) -> Result<R, ArchiveError> {
    let mut archive = tar::Archive::new(decoded);
    let mut names = HashSet::new();
    let mut headers = 0u64;
    let mut members = 0u64;
    let mut pending = Pending::default();
    for entry in archive.entries()?.raw(true) {
        let mut entry = entry?;
        headers += 1;
        if headers > limits.tar_headers() {
            return Err(Refusal::TooManyEntries {
                limit: limits.tar_headers(),
            }
            .into());
        }
        let header = *entry.header().as_bytes();
        let size = entry.size();
        match header[156] {
            b'x' => {
                let data = record(&mut entry, size)?;
                for (keyword, value) in pax_records(&data)? {
                    match keyword {
                        b"path" if pending.name.is_none() => pending.name = Some(value.to_vec()),
                        b"size" => pending.pax_size = true,
                        _ if keyword.starts_with(b"GNU.sparse.") => pending.sparse = true,
                        _ => {}
                    }
                }
                pending.headers += 1;
                continue;
            }
            b'L' => {
                let data = record(&mut entry, size)?;
                if pending.name.is_none() {
                    pending.name = Some(until_nul(&data).to_vec());
                }
                pending.headers += 1;
                continue;
            }
            b'K' => {
                record(&mut entry, size)?;
                pending.headers += 1;
                continue;
            }
            b'g' => return Err(Refusal::GlobalHeader.into()),
            _ => {}
        }
        let current = std::mem::take(&mut pending);
        let name = current.name.unwrap_or_else(|| header_name(&header));
        if current.sparse || header[156] == b'S' {
            return Err(Refusal::Sparse { name }.into());
        }
        if current.pax_size {
            return Err(Refusal::PaxSize { name }.into());
        }
        let directory = header[156] == 0 && until_nul(&header[..100]).ends_with(b"/");
        if !matches!(header[156], b'0' | 0 | b'7') || directory {
            let kind = if directory {
                "directory"
            } else {
                member_kind(header[156])
            };
            return Err(Refusal::NotRegular {
                name,
                kind: kind.to_string(),
            }
            .into());
        }
        let mode = entry
            .header()
            .mode()
            .map_err(|error| Refusal::Malformed(error.to_string()))?;
        if mode & !0o777 != 0 {
            return Err(Refusal::SpecialMode { name, mode }.into());
        }
        members += 1;
        if members > limits.members {
            return Err(Refusal::TooManyEntries {
                limit: limits.members,
            }
            .into());
        }
        if !names.insert(name.clone()) {
            return Err(Refusal::Duplicate { name }.into());
        }
        let member = TarMember { name, size, mode };
        visit(
            &member,
            &mut Exact {
                inner: &mut entry,
                remaining: size,
            },
        )?;
    }
    if pending.headers > 0 {
        return Err(Refusal::Malformed("extension headers describe no member".to_string()).into());
    }
    Ok(archive.into_inner())
}

/// The record framing of `tarfile._create_pax_generic_header`: the length counts its own digits.
fn pax_record(keyword: &[u8], value: &[u8]) -> Vec<u8> {
    let base = keyword.len() + value.len() + 3;
    let mut length = 0;
    loop {
        let next = base + length.to_string().len();
        if next == length {
            break;
        }
        length = next;
    }
    [
        length.to_string().as_bytes(),
        b" ",
        keyword,
        b"=",
        value,
        b"\n",
    ]
    .concat()
}

fn ustar_header(name: &[u8], size: u64, mode: u32, kind: tar::EntryType) -> tar::Header {
    let mut header = tar::Header::new_ustar();
    let field = &mut header.as_old_mut().name;
    let length = name.len().min(field.len());
    field[..length].copy_from_slice(&name[..length]);
    header.set_entry_type(kind);
    header.set_size(size);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_cksum();
    header
}

/// A gzip-compressed tar archive of regular files, written as `evidence_store.pack` writes one: level 9, no file
/// name, modification time 0, and a PAX `path` record for a name over 100 bytes.
pub struct PackageWriter<W: Write> {
    builder: tar::Builder<GzEncoder<W>>,
}

impl<W: Write> PackageWriter<W> {
    pub fn new(out: W) -> Self {
        Self {
            builder: tar::Builder::new(gzip_writer(out, 9)),
        }
    }

    /// Append `size` bytes of `data` as a regular member; `data` must hold at least that many.
    pub fn append(
        &mut self,
        name: &[u8],
        size: u64,
        mode: u32,
        data: &mut dyn Read,
    ) -> Result<(), ArchiveError> {
        if mode & !0o777 != 0 {
            return Err(Refusal::SpecialMode {
                name: name.to_vec(),
                mode,
            }
            .into());
        }
        if name.is_empty() || name.contains(&0) {
            return Err(Refusal::Malformed(format!("cannot name a member {name:?}")).into());
        }
        if name.len() > 100 {
            let record = pax_record(b"path", name);
            let header = ustar_header(
                b"././@PaxHeader",
                record.len() as u64,
                0,
                tar::EntryType::XHeader,
            );
            self.builder.append(&header, record.as_slice())?;
        }
        let header = ustar_header(name, size, mode, tar::EntryType::Regular);
        self.builder.append(
            &header,
            Exact {
                inner: data,
                remaining: size,
            },
        )?;
        Ok(())
    }

    pub fn finish(self) -> Result<W, ArchiveError> {
        Ok(self.builder.into_inner()?.finish()?)
    }
}

/// A regular member of a zip archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipMember {
    pub name: Vec<u8>,
    pub size: u64,
}

fn u16_at(bytes: &[u8], at: usize) -> u64 {
    u64::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> u64 {
    u64::from(u32::from_le_bytes([
        bytes[at],
        bytes[at + 1],
        bytes[at + 2],
        bytes[at + 3],
    ]))
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(word)
}

fn read_at<R: Read + Seek>(file: &mut R, at: u64, buf: &mut [u8]) -> io::Result<bool> {
    file.seek(SeekFrom::Start(at))?;
    match file.read_exact(buf) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

/// The first zip64 end record at or after `from` and before `to`, where `ZipArchive` looks for it.
fn first_zip64_end<R: Read + Seek>(
    file: &mut R,
    from: u64,
    to: u64,
) -> io::Result<Option<[u8; 56]>> {
    let mut at = from;
    let mut chunk = vec![0u8; 64 * 1024];
    while at + 4 <= to {
        let length = usize::try_from((to - at).min(chunk.len() as u64)).unwrap_or(chunk.len());
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut chunk[..length])?;
        if let Some(offset) = chunk[..length]
            .windows(4)
            .position(|window| window == ZIP64_END_SIGNATURE)
        {
            let mut record = [0u8; 56];
            return Ok(read_at(file, at + offset as u64, &mut record)?.then_some(record));
        }
        if length < 4 {
            break;
        }
        at += (length - 3) as u64;
    }
    Ok(None)
}

/// Refuse a central directory above `limits` before `ZipArchive` reads it. Every end record in the comment window
/// is checked, because `ZipArchive` tries each in turn.
fn check_zip_end_records<R: Read + Seek>(
    file: &mut R,
    limits: &Limits,
) -> Result<(), ArchiveError> {
    let length = file.seek(SeekFrom::End(0))?;
    let start = length.saturating_sub(ZIP_END + ZIP_COMMENT_LIMIT + 1);
    let mut tail = vec![0u8; usize::try_from(length - start).unwrap_or(0)];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut tail)?;
    let mut found = false;
    for at in (0..tail.len().saturating_sub(ZIP_END as usize - 1)).rev() {
        if tail[at..at + 4] != ZIP_END_SIGNATURE {
            continue;
        }
        found = true;
        let record = &tail[at..at + ZIP_END as usize];
        let (entries32, bytes32) = (
            u16_at(record, 8).max(u16_at(record, 10)),
            u32_at(record, 12),
        );
        let position = start + at as u64;
        let mut zip64 = None;
        let mut locator = [0u8; 20];
        if position >= 20
            && read_at(file, position - 20, &mut locator)?
            && locator[..4] == ZIP64_LOCATOR_SIGNATURE
        {
            let offset = u64_at(&locator, 8);
            if offset < position - 20 {
                zip64 = first_zip64_end(file, offset, position - 20)?;
            }
        }
        let mut claims = Vec::new();
        if let Some(end) = zip64 {
            claims.push((u64_at(&end, 24).max(u64_at(&end, 32)), u64_at(&end, 40)));
        }
        claims.push((
            if zip64.is_some() && entries32 == 0xFFFF {
                0
            } else {
                entries32
            },
            if zip64.is_some() && bytes32 == 0xFFFF_FFFF {
                0
            } else {
                bytes32
            },
        ));
        for (entries, bytes) in claims {
            if entries > limits.members || bytes > limits.zip_directory_bytes() {
                return Err(Refusal::ZipDirectory { entries, bytes }.into());
            }
        }
    }
    if !found {
        return Err(
            Refusal::Malformed("no zip end of central directory record".to_string()).into(),
        );
    }
    Ok(())
}

/// Read a zip archive, giving each member and its decompressed data to `visit` in central directory order.
pub fn read_zip<R: Read + Seek>(
    mut file: R,
    limits: &Limits,
    mut visit: impl FnMut(&ZipMember, &mut dyn Read) -> Result<(), ArchiveError>,
) -> Result<(), ArchiveError> {
    check_zip_end_records(&mut file, limits)?;
    let mut archive = ZipArchive::new(file)?;
    if archive.len() as u64 > limits.members {
        return Err(Refusal::TooManyEntries {
            limit: limits.members,
        }
        .into());
    }
    if let Err(error) = archive.validate_header_consistency() {
        if error.to_string().contains("duplicate file names") {
            return Err(Refusal::DuplicateZipEntries.into());
        }
        return Err(error.into());
    }
    for index in 0..archive.len() {
        let (name, size) = {
            let entry = archive.by_index_data(index)?;
            let name = entry.name_raw().to_vec();
            if entry.encrypted() {
                return Err(Refusal::Encrypted { name }.into());
            }
            let method = entry.compression();
            if !matches!(
                method,
                CompressionMethod::Stored | CompressionMethod::Deflated
            ) {
                return Err(Refusal::Method {
                    name,
                    method: method.to_string(),
                }
                .into());
            }
            let file_type = entry.unix_mode().map_or(0, |mode| mode & 0o170_000);
            if entry.is_dir() || file_type == 0o040_000 {
                return Err(Refusal::NotRegular {
                    name,
                    kind: "directory".to_string(),
                }
                .into());
            }
            if file_type != 0 && file_type != 0o100_000 {
                let kind = if file_type == 0o120_000 {
                    "symbolic link"
                } else {
                    "special file"
                };
                return Err(Refusal::NotRegular {
                    name,
                    kind: kind.to_string(),
                }
                .into());
            }
            (name, entry.size())
        };
        let mut data = archive.by_index(index)?;
        visit(
            &ZipMember { name, size },
            &mut Exact {
                inner: &mut data,
                remaining: size,
            },
        )?;
        io::copy(&mut data, &mut io::sink())?;
    }
    Ok(())
}

/// A native scratch archive as `native_artifacts.file_artifacts` writes one: deflate entries named by content hash,
/// mode 0600 and the 1980-01-01 timestamp.
pub struct ScratchWriter<W: Write + Seek> {
    zip: ZipWriter<W>,
    names: HashSet<String>,
}

impl<W: Write + Seek> ScratchWriter<W> {
    pub fn new(out: W) -> Self {
        Self {
            zip: ZipWriter::new(out),
            names: HashSet::new(),
        }
    }

    pub fn append(&mut self, name: &str, data: &mut dyn Read) -> Result<(), ArchiveError> {
        if !self.names.insert(name.to_string()) {
            return Err(Refusal::Duplicate {
                name: name.as_bytes().to_vec(),
            }
            .into());
        }
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(6))
            .system(System::Unix)
            .external_attributes(0o600 << 16)
            .last_modified_time(DateTime::default());
        self.zip.start_file(name, options)?;
        io::copy(data, &mut self.zip)?;
        Ok(())
    }

    pub fn finish(self) -> Result<W, ArchiveError> {
        Ok(self.zip.finish()?)
    }
}

/// A member as an evidence manifest indexes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedMember {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub mode: u32,
}

/// A package as an evidence manifest indexes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedPackage {
    pub asset: String,
    pub sha256: String,
    pub bytes: u64,
    pub members: Vec<IndexedMember>,
}

fn field<'a>(
    object: &'a crate::pyjson::Object,
    key: &str,
) -> Result<&'a crate::pyjson::Value, String> {
    object.get(key).ok_or_else(|| format!("missing {key}"))
}

fn text_field(object: &crate::pyjson::Object, key: &str) -> Result<String, String> {
    field(object, key)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{key} is not a string"))
}

fn count_field(object: &crate::pyjson::Object, key: &str) -> Result<u64, String> {
    match field(object, key)? {
        crate::pyjson::Value::Int(digits) => {
            digits.parse().map_err(|_| format!("{key} is not a count"))
        }
        _ => Err(format!("{key} is not an integer")),
    }
}

/// The packages a manifest indexes, with the fields `check_package` compares.
pub fn indexed_packages(manifest: &crate::pyjson::Value) -> Result<Vec<IndexedPackage>, String> {
    let packages = manifest
        .as_object()
        .and_then(|object| object.get("packages"))
        .and_then(crate::pyjson::Value::as_array)
        .ok_or("the manifest has no packages list")?;
    packages
        .iter()
        .map(|package| {
            let package = package.as_object().ok_or("a package is not an object")?;
            let members = field(package, "members")?
                .as_array()
                .ok_or("members is not a list")?
                .iter()
                .map(|member| {
                    let member = member.as_object().ok_or("a member is not an object")?;
                    Ok(IndexedMember {
                        path: text_field(member, "path")?,
                        sha256: text_field(member, "sha256")?,
                        bytes: count_field(member, "bytes")?,
                        mode: u32::try_from(count_field(member, "mode")?)
                            .map_err(|_| "mode is too large")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(IndexedPackage {
                asset: text_field(package, "asset")?,
                sha256: text_field(package, "sha256")?,
                bytes: count_field(package, "bytes")?,
                members,
            })
        })
        .collect()
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn hash_stream(data: &mut dyn Read) -> io::Result<String> {
    let mut hasher = crate::sha256::Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let read = data.read(&mut buf)?;
        if read == 0 {
            return Ok(crate::sha256::hex(&hasher.finalize()));
        }
        hasher.update(&buf[..read]);
    }
}

/// Check a cached package as `evidence_store.unpack` does, without restoring it: the archive's size and SHA-256
/// first, then each member's name, size, mode and SHA-256 against the index. The error is the diagnostic.
pub fn check_package(cache: &std::path::Path, package: &IndexedPackage) -> Result<(), String> {
    if !is_sha256(&package.sha256) || package.asset != format!("{}.tar.gz", package.sha256) {
        return Err("asset name must identify the archive bytes".to_string());
    }
    let path = cache.join(&package.asset);
    let changed = || format!("missing or changed evidence: {}", path.display());
    let regular =
        std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file());
    if !regular {
        return Err(changed());
    }
    let mut file =
        std::fs::File::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if file.metadata().map_err(|error| error.to_string())?.len() != package.bytes
        || hash_stream(&mut file).map_err(|error| error.to_string())? != package.sha256
    {
        return Err(changed());
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let expected: std::collections::HashMap<&[u8], &IndexedMember> = package
        .members
        .iter()
        .map(|member| (member.path.as_bytes(), member))
        .collect();
    let limits = Limits {
        members: package.members.len() as u64,
        payload: package.members.iter().map(|member| member.bytes).sum(),
    };
    let mut seen = 0usize;
    let mut mismatch = None;
    let result = read_tar_gz(file, &limits, |member, data| {
        let shown = shown(&member.name);
        let problem = match expected.get(member.name.as_slice()) {
            None => Some(format!("unexpected archive member: {shown}")),
            Some(indexed) if indexed.bytes != member.size || indexed.mode != member.mode => {
                Some(format!("archive metadata differs: {shown}"))
            }
            Some(indexed) if hash_stream(data)? != indexed.sha256 => {
                Some(format!("missing or changed evidence: {shown}"))
            }
            Some(_) => None,
        };
        seen += 1;
        match problem {
            Some(problem) => {
                mismatch = Some(problem);
                Err(Refusal::Malformed("the archive differs from its index".to_string()).into())
            }
            None => Ok(()),
        }
    });
    match (result, mismatch) {
        (_, Some(problem)) => Err(problem),
        (Err(error), None) => Err(error.to_string()),
        (Ok(()), None) if seen != package.members.len() => {
            Err("archive is missing indexed members".to_string())
        }
        (Ok(()), None) => Ok(()),
    }
}

/// Check a native scratch archive as `native_artifacts.verify` reads one: each entry's SHA-256 is its name.
/// Without the attempt's index, the entry limit is what the archive's size can hold. Returns the entry count.
pub fn check_scratch(path: &std::path::Path) -> Result<u64, String> {
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let length = file.metadata().map_err(|error| error.to_string())?.len();
    let limits = Limits {
        members: length / (30 + ZIP_CENTRAL_ENTRY),
        payload: 0,
    };
    let mut entries = 0;
    let mut mismatch = None;
    let result = read_zip(file, &limits, |member, data| {
        entries += 1;
        let name = shown(&member.name);
        if !is_sha256(&name) || hash_stream(data)? != name {
            mismatch = Some(format!("archived content differs from index: {name}"));
            return Err(
                Refusal::Malformed("the archive differs from its names".to_string()).into(),
            );
        }
        Ok(())
    });
    match (result, mismatch) {
        (_, Some(problem)) => Err(problem),
        (Err(error), None) => Err(error.to_string()),
        (Ok(()), None) => Ok(entries),
    }
}
