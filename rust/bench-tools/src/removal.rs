//! Removal of a verified directory tree that a substituted path cannot redirect. Every directory is opened relative
//! to its already opened parent without following a link, its device and inode are compared with what was verified
//! before anything inside it is touched, and a tree that reaches another filesystem is refused. A directory that
//! cannot be read is an error, never an empty one.

use crate::pytext;
use nix::dir::Dir;
use nix::errno::Errno;
use nix::fcntl::{self, AtFlags, OFlag};
use nix::sys::stat::{self, FchmodatFlags, FileStat, Mode, SFlag};
use nix::unistd::{self, UnlinkatFlags};
use std::ffi::OsStr;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static ASIDE: AtomicU64 = AtomicU64::new(0);

/// A file's device and inode, which a path swapped after verification cannot keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub device: u64,
    pub inode: u64,
}

impl Identity {
    #[allow(
        clippy::unnecessary_cast,
        reason = "the types of st_dev and st_ino differ by platform"
    )]
    fn of(stat: &FileStat) -> Self {
        Self {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        }
    }
}

/// The places a test interrupts a removal, to prove a failure never removes more than the receipt says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    /// The journal is written and nothing is removed yet.
    BeforeRemoval,
    /// One entry was just removed.
    Removed,
    /// Every candidate is removed and the receipt is not yet written.
    BeforeReceipt,
}

/// What `check` sees of an entry about to be unlinked.
pub enum Entry<'a> {
    File(&'a std::fs::File),
    /// A link, which is never opened through, or another kind of file, which is never opened.
    Other(Kind),
}

/// Sees each entry that is not a directory, by its path below the removed directory, before it is unlinked; an
/// error stops the removal.
pub type Check<'a> = &'a mut dyn FnMut(&[u8], Entry<'_>) -> Result<(), String>;

fn reborrow<'a>(check: &'a mut Option<Check<'_>>) -> Option<Check<'a>> {
    match check {
        Some(check) => Some(&mut **check),
        None => None,
    }
}

/// Called at each [`Point`]; an error stops the removal there. Production passes [`uninterrupted`].
pub type Hook<'a> = &'a dyn Fn(Point) -> std::io::Result<()>;

pub fn uninterrupted(_: Point) -> std::io::Result<()> {
    Ok(())
}

fn failure(errno: Errno, path: &Path) -> String {
    pytext::os_error(
        &std::io::Error::from_raw_os_error(errno as i32),
        path.as_os_str().as_bytes(),
    )
}

fn is(stat: &FileStat, kind: SFlag) -> bool {
    SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT == kind
}

/// The bytes an unprivileged user may still write on the filesystem holding `path`.
pub fn free_bytes(path: &Path) -> Result<u64, String> {
    let stats = nix::sys::statvfs::statvfs(path).map_err(|errno| failure(errno, path))?;
    #[allow(
        clippy::unnecessary_cast,
        clippy::useless_conversion,
        reason = "the types of the statvfs fields differ by platform"
    )]
    Ok(u64::from(stats.blocks_available()) * u64::from(stats.fragment_size()))
}

/// What a path names now, without following a final link; `None` when nothing is there.
pub fn lstat(path: &Path) -> Result<Option<(Identity, Kind)>, String> {
    match stat::lstat(path) {
        Ok(stat) => Ok(Some((Identity::of(&stat), Kind::of(&stat)))),
        Err(Errno::ENOENT) => Ok(None),
        Err(errno) => Err(failure(errno, path)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Directory,
    File,
    Symlink,
    Other,
}

impl Kind {
    fn of(stat: &FileStat) -> Self {
        if is(stat, SFlag::S_IFDIR) {
            Self::Directory
        } else if is(stat, SFlag::S_IFREG) {
            Self::File
        } else if is(stat, SFlag::S_IFLNK) {
            Self::Symlink
        } else {
            Self::Other
        }
    }
}

/// An open directory and the path it was opened at, for messages.
pub struct Directory {
    fd: OwnedFd,
    path: std::path::PathBuf,
    identity: Identity,
}

const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);

impl Directory {
    /// Open `path`, which must still be the directory `expected` names; its final component is never followed.
    pub fn open(path: &Path, expected: Identity) -> Result<Self, String> {
        let fd =
            fcntl::open(path, DIRECTORY, Mode::empty()).map_err(|errno| failure(errno, path))?;
        let identity = Identity::of(&stat::fstat(&fd).map_err(|errno| failure(errno, path))?);
        if identity != expected {
            return Err(format!("{} changed after it was verified", path.display()));
        }
        Ok(Self {
            fd,
            path: path.to_path_buf(),
            identity,
        })
    }

    pub fn identity(&self) -> Identity {
        self.identity
    }

    /// What `name` inside this directory is now, without following a link; `None` when it is absent.
    pub fn entry(&self, name: &[u8]) -> Result<Option<(Identity, Kind)>, String> {
        let path = self.path.join(OsStr::from_bytes(name));
        match stat::fstatat(
            &self.fd,
            OsStr::from_bytes(name),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => Ok(Some((Identity::of(&stat), Kind::of(&stat)))),
            Err(Errno::ENOENT) => Ok(None),
            Err(errno) => Err(failure(errno, &path)),
        }
    }

    /// Unlink the regular file `name` when it is present; anything else there is an error.
    pub fn unlink_file(&self, name: &[u8]) -> Result<(), String> {
        match self.entry(name)? {
            None => Ok(()),
            Some((_, Kind::File)) => unistd::unlinkat(
                &self.fd,
                OsStr::from_bytes(name),
                UnlinkatFlags::NoRemoveDir,
            )
            .map_err(|errno| failure(errno, &self.path.join(OsStr::from_bytes(name)))),
            Some(_) => Err(format!(
                "{} is not a regular file",
                self.path.join(OsStr::from_bytes(name)).display()
            )),
        }
    }

    /// Remove the directory `name` and everything below it, once it is checked to still be `expected`. Without a
    /// `check`, no file is opened, so a file its owner cannot read is still removed.
    pub fn remove_tree(
        &self,
        name: &[u8],
        expected: Identity,
        check: Option<Check<'_>>,
        hook: Hook<'_>,
    ) -> Result<(), String> {
        let stat = stat::fstatat(
            &self.fd,
            OsStr::from_bytes(name),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        )
        .map_err(|errno| failure(errno, &self.path.join(OsStr::from_bytes(name))))?;
        if !is(&stat, SFlag::S_IFDIR) || Identity::of(&stat) != expected {
            return Err(format!(
                "{} changed after it was verified",
                self.path.join(OsStr::from_bytes(name)).display()
            ));
        }
        self.remove_directory(name, &stat, b"", check, hook)
    }

    fn remove_directory(
        &self,
        name: &[u8],
        stat: &FileStat,
        relative: &[u8],
        mut check: Option<Check<'_>>,
        hook: Hook<'_>,
    ) -> Result<(), String> {
        let path = self.path.join(OsStr::from_bytes(name));
        if stat.st_mode & 0o500 != 0o500 {
            return Err(failure(Errno::EACCES, &path));
        }
        if stat.st_mode & 0o200 == 0 {
            stat::fchmodat(
                &self.fd,
                OsStr::from_bytes(name),
                Mode::from_bits_truncate(stat.st_mode | 0o200),
                FchmodatFlags::NoFollowSymlink,
            )
            .map_err(|errno| failure(errno, &path))?;
        }
        let fd = fcntl::openat(&self.fd, OsStr::from_bytes(name), DIRECTORY, Mode::empty())
            .map_err(|errno| failure(errno, &path))?;
        let opened = stat::fstat(&fd).map_err(|errno| failure(errno, &path))?;
        if Identity::of(&opened) != Identity::of(stat) {
            return Err(format!("{} changed during removal", path.display()));
        }
        let mut listing = Dir::from_fd(
            fd.try_clone()
                .map_err(|error| pytext::os_error(&error, path.as_os_str().as_bytes()))?,
        )
        .map_err(|errno| failure(errno, &path))?;
        let mut names = Vec::new();
        for entry in listing.iter() {
            let entry = entry.map_err(|errno| failure(errno, &path))?;
            let child = entry.file_name().to_bytes();
            if child != b"." && child != b".." {
                names.push(child.to_vec());
            }
        }
        drop(listing);
        let directory = Self {
            fd,
            path: path.clone(),
            identity: Identity::of(&opened),
        };
        for child in names {
            let below = if relative.is_empty() {
                child.clone()
            } else {
                [relative, b"/", &child].concat()
            };
            let child_path = path.join(OsStr::from_bytes(&child));
            let child_stat = stat::fstatat(
                &directory.fd,
                OsStr::from_bytes(&child),
                AtFlags::AT_SYMLINK_NOFOLLOW,
            )
            .map_err(|errno| failure(errno, &child_path))?;
            if is(&child_stat, SFlag::S_IFDIR) {
                if Identity::of(&child_stat).device != directory.identity.device {
                    return Err(format!(
                        "{} is on another filesystem; it is never removed",
                        child_path.display()
                    ));
                }
                directory.remove_directory(
                    &child,
                    &child_stat,
                    &below,
                    reborrow(&mut check),
                    hook,
                )?;
                continue;
            }
            if let Some(check) = reborrow(&mut check) {
                directory.check_entry(&child, &child_stat, &below, check)?;
            }
            directory.unlink_checked(&child, &child_stat)?;
            hook(Point::Removed).map_err(|error| error.to_string())?;
        }
        drop(directory);
        self.still(name, stat)?;
        unistd::unlinkat(&self.fd, OsStr::from_bytes(name), UnlinkatFlags::RemoveDir)
            .map_err(|errno| failure(errno, &path))?;
        hook(Point::Removed).map_err(|error| error.to_string())
    }

    /// Unlink `name` only if it is still the entry `stat` describes. It is first renamed to a name of its own, so
    /// one renamed into its place meanwhile is what gets checked and is kept, under the reported name.
    fn unlink_checked(&self, name: &[u8], stat: &FileStat) -> Result<(), String> {
        let path = self.path.join(OsStr::from_bytes(name));
        let aside = format!(
            ".bench-tools-removing-{}-{}",
            std::process::id(),
            ASIDE.fetch_add(1, Ordering::Relaxed)
        );
        let aside_path = self.path.join(&aside);
        if self.entry(aside.as_bytes())?.is_some() {
            return Err(format!(
                "{} already exists; nothing is removed in its place",
                aside_path.display()
            ));
        }
        fcntl::renameat(
            &self.fd,
            OsStr::from_bytes(name),
            &self.fd,
            OsStr::new(&aside),
        )
        .map_err(|errno| failure(errno, &path))?;
        if self.entry(aside.as_bytes())? != Some((Identity::of(stat), Kind::of(stat))) {
            return Err(format!(
                "{} changed during removal; what replaced it is kept at {}",
                path.display(),
                aside_path.display()
            ));
        }
        unistd::unlinkat(&self.fd, OsStr::new(&aside), UnlinkatFlags::NoRemoveDir)
            .map_err(|errno| failure(errno, &aside_path))
    }

    /// `name` must still be the entry `stat` describes, so one renamed into its place while it was checked stays.
    fn still(&self, name: &[u8], stat: &FileStat) -> Result<(), String> {
        let path = self.path.join(OsStr::from_bytes(name));
        let now = stat::fstatat(
            &self.fd,
            OsStr::from_bytes(name),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        )
        .map_err(|errno| failure(errno, &path))?;
        if Identity::of(&now) != Identity::of(stat) || Kind::of(&now) != Kind::of(stat) {
            return Err(format!("{} changed during removal", path.display()));
        }
        Ok(())
    }

    /// Show an entry to `check`: a regular file through a descriptor that is still the entry just listed, anything
    /// else unopened.
    fn check_entry(
        &self,
        name: &[u8],
        stat: &FileStat,
        relative: &[u8],
        check: Check<'_>,
    ) -> Result<(), String> {
        if !is(stat, SFlag::S_IFREG) {
            return check(relative, Entry::Other(Kind::of(stat)));
        }
        let path = self.path.join(OsStr::from_bytes(name));
        let flags = OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK;
        let file = fcntl::openat(&self.fd, OsStr::from_bytes(name), flags, Mode::empty())
            .map(std::fs::File::from)
            .map_err(|errno| failure(errno, &path))?;
        let opened = stat::fstat(file.as_fd()).map_err(|errno| failure(errno, &path))?;
        if Identity::of(&opened) != Identity::of(stat) {
            return Err(format!("{} changed during removal", path.display()));
        }
        check(relative, Entry::File(&file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "bench-tools-removal-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("tree")).unwrap();
        path
    }

    fn identity(path: &Path) -> Identity {
        lstat(path).unwrap().unwrap().0
    }

    #[test]
    fn a_file_renamed_over_a_checked_name_is_never_unlinked() {
        let base = scratch("renamed");
        fs::write(base.join("tree/checked"), "verified").unwrap();
        fs::write(base.join("substitute"), "never verified").unwrap();
        let parent = Directory::open(&base, identity(&base)).unwrap();
        let (tree, substitute) = (base.join("tree"), base.join("substitute"));
        let result = parent.remove_tree(
            b"tree",
            identity(&tree),
            Some(&mut |_, _| {
                fs::rename(&substitute, tree.join("checked")).map_err(|error| error.to_string())
            }),
            &uninterrupted,
        );
        let kept = match &result {
            Err(message) => message
                .split_once("; what replaced it is kept at ")
                .map(|(_, aside)| fs::read_to_string(aside)),
            Ok(()) => None,
        };
        fs::remove_dir_all(&base).unwrap();
        assert_eq!(
            kept.map(Result::unwrap).as_deref(),
            Some("never verified"),
            "{result:?}"
        );
    }

    #[test]
    fn an_unreadable_directory_is_refused_and_a_read_only_one_is_removed() {
        use std::os::unix::fs::PermissionsExt;
        let base = scratch("unreadable");
        let parent = Directory::open(&base, identity(&base)).unwrap();
        let locked = base.join("tree/locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("unchecked"), "kept").unwrap();
        for mode in [0o000, 0o300, 0o600] {
            fs::set_permissions(&locked, fs::Permissions::from_mode(mode)).unwrap();
            let result =
                parent.remove_tree(b"tree", identity(&base.join("tree")), None, &uninterrupted);
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(
                matches!(&result, Err(message) if message.contains("Permission denied")),
                "{mode:o}: {result:?}"
            );
            assert_eq!(
                fs::read_to_string(locked.join("unchecked")).unwrap(),
                "kept"
            );
        }
        fs::set_permissions(locked.join("unchecked"), fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let result =
            parent.remove_tree(b"tree", identity(&base.join("tree")), None, &uninterrupted);
        let left = fs::symlink_metadata(base.join("tree")).is_ok();
        fs::remove_dir_all(&base).unwrap();
        assert_eq!(result, Ok(()));
        assert!(!left);
    }
}
