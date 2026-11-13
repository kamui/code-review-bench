//! A `claude-skill` arm's `step_models`: the exact model each model alias the skill's prompt names runs on.
//!
//! Claude Code resolves the aliases `haiku`, `sonnet` and `opus`, which its Agent tool takes as a subagent's model,
//! through `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL` and `ANTHROPIC_DEFAULT_OPUS_MODEL`. An arm
//! without the map keeps `CLAUDE_CODE_SUBAGENT_MODEL`, which puts every subagent on the arm's model.
//!
//! A subagent is bound to the Agent call that started it: the tool result of that call names the agent's id, which
//! is the name of its transcript, `agent-<id>.jsonl`. The call must name a mapped alias, and the transcript must run
//! that alias's model and no other.

use crate::pyjson::{Object, Value};
use std::collections::BTreeMap;

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

    /// Why the subagent `agent`, whose transcript records `models`, broke the map, or `None` when the Agent call
    /// that started it named a mapped alias and the transcript ran only that alias's model.
    pub fn subagent_problem(
        &self,
        agent: Option<&str>,
        models: &[&str],
        started: &BTreeMap<String, Value>,
    ) -> Option<String> {
        let Some(requested) = agent.and_then(|agent| started.get(agent)) else {
            return Some("no Agent call that started it was found".to_string());
        };
        let Some((alias, model)) = requested
            .as_str()
            .and_then(|alias| self.0.iter().find(|(mapped, _)| *mapped == alias))
        else {
            return Some(format!(
                "its Agent call asked for model {}, not a mapped alias ({})",
                requested.py_repr(),
                self.describe()
            ));
        };
        (models != [model.as_str()]).then(|| {
            format!(
                "it ran {}, and its Agent call asked for {alias}, which maps to {model}",
                models.join(", ")
            )
        })
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

/// The id of the subagent whose transcript is named `name`, `agent-<id>.jsonl`.
pub fn agent_id(name: &str) -> Option<&str> {
    name.strip_prefix("agent-")?
        .strip_suffix(".jsonl")
        .filter(|id| !id.is_empty())
}

/// The `model` argument of the Agent call that started each subagent, by agent id, from the Agent calls and their
/// tool results in `records`, which may come from any transcript of the attempt. A call without the argument maps
/// to null.
pub fn started_models<'a>(records: impl IntoIterator<Item = &'a Value>) -> BTreeMap<String, Value> {
    let field = |value: &'a Value, key: &str| value.as_object().and_then(|object| object.get(key));
    let mut calls: BTreeMap<String, Value> = BTreeMap::new();
    let mut agents: Vec<(String, String)> = Vec::new();
    for record in records {
        let message = field(record, "message");
        let blocks = message
            .and_then(|message| field(message, "content"))
            .and_then(Value::as_array)
            .unwrap_or_default();
        match field(record, "type").and_then(Value::as_str) {
            Some("assistant") => {
                for block in blocks {
                    let name = field(block, "name").and_then(Value::as_str);
                    if field(block, "type").and_then(Value::as_str) == Some("tool_use")
                        && matches!(name, Some("Agent" | "Task"))
                        && let Some(id) = field(block, "id").and_then(Value::as_str)
                    {
                        let model = field(block, "input")
                            .and_then(|input| field(input, "model"))
                            .cloned()
                            .unwrap_or(Value::Null);
                        calls.insert(id.to_string(), model);
                    }
                }
            }
            Some("user") => {
                let Some(agent) = field(record, "toolUseResult")
                    .and_then(|result| field(result, "agentId"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                for block in blocks {
                    if let Some(id) = field(block, "tool_use_id").and_then(Value::as_str) {
                        agents.push((agent.to_string(), id.to_string()));
                    }
                }
            }
            _ => {}
        }
    }
    agents
        .into_iter()
        .filter_map(|(agent, id)| calls.get(&id).map(|model| (agent, model.clone())))
        .collect()
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
    fn a_subagent_must_run_the_model_its_agent_call_named() {
        let map = read(&arm(
            r#", "step_models": {"haiku": "claude-haiku-5-5", "opus": "claude-opus-5-5"}"#,
        ))
        .unwrap()
        .unwrap();
        let records: Vec<Value> = [
            r#"{"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Agent", "input": {"model": "haiku"}}, {"type": "tool_use", "id": "t2", "name": "Agent", "input": {}}, {"type": "tool_use", "id": "t3", "name": "Agent", "input": {"model": "sonnet"}}]}}"#,
            r#"{"type": "user", "toolUseResult": {"agentId": "a1"}, "message": {"content": [{"type": "tool_result", "tool_use_id": "t1"}]}}"#,
            r#"{"type": "user", "toolUseResult": {"agentId": "a2"}, "message": {"content": [{"type": "tool_result", "tool_use_id": "t2"}]}}"#,
            r#"{"type": "user", "toolUseResult": {"agentId": "a3"}, "message": {"content": [{"type": "tool_result", "tool_use_id": "t3"}]}}"#,
        ]
        .iter()
        .map(|line| pyjson::loads(line, Duplicates::KeepLast).unwrap())
        .collect();
        let started = started_models(&records);
        assert_eq!(agent_id("agent-a1.jsonl"), Some("a1"));
        assert_eq!(
            map.subagent_problem(Some("a1"), &["claude-haiku-5-5"], &started),
            None
        );
        for (agent, models, expected) in [
            (
                "a1",
                &["claude-opus-5-5"][..],
                "it ran claude-opus-5-5, and its Agent call asked for haiku",
            ),
            (
                "a1",
                &["claude-haiku-5-5", "claude-opus-5-5"][..],
                "asked for haiku, which maps to claude-haiku-5-5",
            ),
            (
                "a2",
                &["claude-opus-5-5"][..],
                "asked for model None, not a mapped alias",
            ),
            (
                "a3",
                &["claude-sonnet-5-5"][..],
                "asked for model 'sonnet', not a mapped alias",
            ),
            (
                "a9",
                &["claude-haiku-5-5"][..],
                "no Agent call that started it",
            ),
        ] {
            let problem = map.subagent_problem(Some(agent), models, &started).unwrap();
            assert!(problem.contains(expected), "{agent}: {problem}");
        }
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
