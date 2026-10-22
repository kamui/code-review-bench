//! A fake release for cargo tests of the commands that restore or publish evidence.

use super::{Remote, Result, Upload};
use crate::pyjson::{self, Duplicates, Value};
use crate::pytext::PyPath;
use std::io::Write;

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

/// A release that keeps every uploaded asset.
#[derive(Default)]
pub struct Release {
    pub assets: Vec<(u64, String, Vec<u8>)>,
}

impl Remote for Release {
    fn release(&mut self, _repository: &str, _tag: &str) -> Result<Value> {
        let assets: Vec<String> = self
            .assets
            .iter()
            .map(|(id, name, data)| {
                format!(
                    r#"{{"id": {id}, "name": "{name}", "size": {}}}"#,
                    data.len()
                )
            })
            .collect();
        Ok(json(&format!(
            r#"{{"id": 7, "draft": false, "assets": [{}]}}"#,
            assets.join(", ")
        )))
    }

    fn download(
        &mut self,
        _repository: &str,
        asset_id: &str,
        destination: &mut dyn Write,
        _limit: u64,
    ) -> Result<()> {
        let (_, _, data) = self
            .assets
            .iter()
            .find(|(id, ..)| id.to_string() == asset_id)
            .unwrap();
        destination.write_all(data).unwrap();
        Ok(())
    }

    fn upload(&mut self, _repository: &str, _tag: &str, archive: &PyPath) -> Result<Upload> {
        let path = archive.to_path_buf();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let id = 101 + self.assets.len() as u64;
        self.assets.push((id, name, std::fs::read(&path).unwrap()));
        Ok(Upload::Done)
    }
}
