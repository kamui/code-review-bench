//! A `claude-skill` arm's `step_models`: the exact model each model alias the skill's prompt names runs on.
//!
//! Claude Code resolves the aliases `haiku`, `sonnet` and `opus`, which its Agent tool takes as a subagent's model,
//! through `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL` and `ANTHROPIC_DEFAULT_OPUS_MODEL`. An arm
//! without the map keeps `CLAUDE_CODE_SUBAGENT_MODEL`, which puts every subagent on the arm's model.

use crate::pyjson::{Object, Value};

pub const ALIASES: [&str; 3] = ["haiku", "sonnet", "opus"];

/// The map, one entry per alias the arm names, in [`ALIASES`] order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepModels(Vec<(&'static str, String)>);

/// The arm's map, or `None` when it names none.
pub fn read(arm: &Value) -> Result<Option<StepModels>, String> {
    let configured = match arm.as_object().and_then(|arm| arm.get("step_models")) {
        None | Some(Value::Null) => return Ok(None),
        Some(configured) => configured,
    };
    if arm
        .as_object()
        .and_then(|arm| arm.get("kind"))
        .and_then(Value::as_str)
        != Some("claude-skill")
    {
        return Err("step_models applies only to a claude-skill arm".to_string());
    }
    let Some(entries) = configured.as_object() else {
        return Err("step_models must map model aliases to model ids".to_string());
    };
    if entries.is_empty() {
        return Err("step_models names no model alias".to_string());
    }
    if let Some((alias, _)) = entries
        .iter()
        .find(|(alias, _)| !ALIASES.contains(&alias.as_str()))
    {
        return Err(format!(
            "step_models names {alias}, which is not one of {}",
            ALIASES.join(", ")
        ));
    }
    let mut mapped = Vec::new();
    for alias in ALIASES {
        match entries.get(alias) {
            None => {}
            Some(Value::Str(model)) if !model.trim().is_empty() && model.trim() == model => {
                mapped.push((alias, model.clone()));
            }
            Some(_) => return Err(format!("step_models.{alias} must be a model id")),
        }
    }
    Ok(Some(StepModels(mapped)))
}

impl StepModels {
    /// The variables that resolve each mapped alias to its model.
    pub fn environment(&self) -> Vec<(String, String)> {
        self.0
            .iter()
            .map(|(alias, model)| {
                (
                    format!("ANTHROPIC_DEFAULT_{}_MODEL", alias.to_uppercase()),
                    model.clone(),
                )
            })
            .collect()
    }

    /// The mapped models, each once, sorted.
    pub fn models(&self) -> Vec<&str> {
        let mut models: Vec<&str> = self.0.iter().map(|(_, model)| model.as_str()).collect();
        models.sort_unstable();
        models.dedup();
        models
    }

    pub fn accepts(&self, model: &str) -> bool {
        self.0.iter().any(|(_, mapped)| mapped == model)
    }

    /// `haiku=<model> sonnet=<model> ...`, for a dispatch record or a message.
    pub fn describe(&self) -> String {
        self.0
            .iter()
            .map(|(alias, model)| format!("{alias}={model}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn to_value(&self) -> Value {
        let mut entries = Object::default();
        for (alias, model) in &self.0 {
            entries.insert((*alias).to_string(), Value::Str(model.clone()));
        }
        Value::Object(entries)
    }

    /// The models an attempt may be priced at: the arm's model, then each mapped one it does not repeat.
    pub fn priced(&self, arm_model: &str) -> Vec<String> {
        let mut models = vec![arm_model.to_string()];
        for model in self.models() {
            if model != arm_model {
                models.push(model.to_string());
            }
        }
        models
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyjson::{self, Duplicates};

    fn arm(fields: &str) -> Value {
        pyjson::loads(
            &format!(r#"{{"kind": "claude-skill"{fields}}}"#),
            Duplicates::KeepLast,
        )
        .unwrap()
    }

    #[test]
    fn an_arm_without_a_map_has_none() {
        assert_eq!(read(&arm("")).unwrap(), None);
        assert_eq!(read(&arm(r#", "step_models": null"#)).unwrap(), None);
    }

    #[test]
    fn each_mapped_alias_gets_its_client_variable() {
        let map = read(&arm(
            r#", "step_models": {"opus": "claude-opus-5-5", "haiku": "claude-haiku-5-5", "sonnet": "claude-sonnet-5-5"}"#,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            map.environment(),
            [
                ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "claude-haiku-5-5"),
                ("ANTHROPIC_DEFAULT_SONNET_MODEL", "claude-sonnet-5-5"),
                ("ANTHROPIC_DEFAULT_OPUS_MODEL", "claude-opus-5-5"),
            ]
            .map(|(name, model)| (name.to_string(), model.to_string()))
        );
        assert!(map.accepts("claude-haiku-5-5") && !map.accepts("claude-haiku-4-5"));
        assert_eq!(
            map.priced("claude-opus-5-5"),
            ["claude-opus-5-5", "claude-haiku-5-5", "claude-sonnet-5-5"]
        );
        assert_eq!(
            map.describe(),
            "haiku=claude-haiku-5-5 sonnet=claude-sonnet-5-5 opus=claude-opus-5-5"
        );
    }

    #[test]
    fn a_malformed_map_is_refused() {
        for (fields, expected) in [
            (r#", "step_models": {}"#, "names no model alias"),
            (
                r#", "step_models": {"fable": "claude-fable-5-1"}"#,
                "not one of",
            ),
            (r#", "step_models": {"haiku": ""}"#, "must be a model id"),
            (r#", "step_models": {"haiku": 5}"#, "must be a model id"),
            (
                r#", "step_models": ["claude-haiku-5-5"]"#,
                "must map model aliases",
            ),
        ] {
            let error = read(&arm(fields)).unwrap_err();
            assert!(error.contains(expected), "{fields}: {error}");
        }
        let codex: Value = pyjson::loads(
            r#"{"kind": "codex-skill", "step_models": {"haiku": "claude-haiku-5-5"}}"#,
            Duplicates::KeepLast,
        )
        .unwrap();
        assert!(
            read(&codex)
                .unwrap_err()
                .contains("only to a claude-skill arm")
        );
    }
}
