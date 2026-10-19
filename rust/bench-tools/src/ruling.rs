//! The checks that keep a ruling with the owner, ported from `bench/tools/ruling_record.py`,
//! `ruling_dossier.py` and `ruling_summary.py`. Nothing here writes a ruling, record or decision; `precedents`
//! writes only the new sheet its command names.

pub mod dossier;
pub mod record;
pub mod summary;

use crate::pyfs;
use crate::pyjson::Value;
use crate::pytext::PyPath;
use crate::pyvalue::{self, Py};
use std::cell::RefCell;
use std::collections::HashMap;

/// The repository a command reads: `ROOT` in the Python, the current directory unless `--root` names another.
/// Files are read once per command; Python rereads them, which is the same while nothing writes them.
pub struct Repo {
    pub root: PyPath,
    loaded: RefCell<HashMap<Vec<u8>, Py<Value>>>,
    digests: RefCell<HashMap<Vec<u8>, Py<String>>>,
}

impl Repo {
    pub fn new(root: &[u8]) -> Self {
        Self {
            root: PyPath::new(&pyfs::resolve(root)),
            loaded: RefCell::default(),
            digests: RefCell::default(),
        }
    }

    /// `root / segment`.
    pub fn path(&self, segment: &Value) -> Py<PyPath> {
        pyvalue::join_path(&self.root, segment)
    }

    pub fn join(&self, segment: &str) -> PyPath {
        self.root.join(segment.as_bytes())
    }

    /// `json.loads(Path(path).read_text(encoding="utf-8"))`.
    pub fn load(&self, path: &PyPath) -> Py<Value> {
        let key = path.as_bytes().to_vec();
        if let Some(value) = self.loaded.borrow().get(&key) {
            return value.clone();
        }
        let value = pyvalue::load(path);
        self.loaded.borrow_mut().insert(key, value.clone());
        value
    }

    /// `hashlib.sha256(Path(path).read_bytes()).hexdigest()`.
    pub fn digest(&self, path: &PyPath) -> Py<String> {
        let key = path.as_bytes().to_vec();
        if let Some(value) = self.digests.borrow().get(&key) {
            return value.clone();
        }
        let value = pyvalue::read_bytes(path).map(|bytes| crate::sha256::hex_digest(&bytes));
        self.digests.borrow_mut().insert(key, value.clone());
        value
    }
}
