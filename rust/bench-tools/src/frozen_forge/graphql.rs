//! Answer a GitHub GraphQL query from a frozen capture.
//!
//! The query's selection set is evaluated against the captured records. A connection is paged from the captured
//! nodes, so `pageInfo` and `totalCount` describe the capture. A field the capture does not hold is an error,
//! except `updatedAt`, which the forge sets on later activity and so is unknown at the cutoff: it is null, as
//! the capture stores an undated thread resolution.

use super::Capture;
use crate::pyjson::{Object, Value};

/// Captured `timelineItems` types, from `forge_capture`'s pull request and issue queries.
pub const PULL_TIMELINE: [&str; 8] = [
    "HEAD_REF_FORCE_PUSHED_EVENT",
    "RENAMED_TITLE_EVENT",
    "READY_FOR_REVIEW_EVENT",
    "CONVERT_TO_DRAFT_EVENT",
    "LABELED_EVENT",
    "UNLABELED_EVENT",
    "BASE_REF_CHANGED_EVENT",
    "REVIEW_DISMISSED_EVENT",
];
pub const ISSUE_TIMELINE: [&str; 3] = ["RENAMED_TITLE_EVENT", "CLOSED_EVENT", "REOPENED_EVENT"];
const UNKNOWN_AT_CUTOFF: [&str; 1] = ["updatedAt"];
const PAGE_LIMIT: usize = 100;

#[derive(Clone, Debug)]
enum Literal {
    Variable(String),
    Int(String),
    Float(f64),
    Str(String),
    Enum(String),
    Bool(bool),
    Null,
    List(Vec<Literal>),
}

#[derive(Debug)]
struct Field {
    key: String,
    name: String,
    arguments: Vec<(String, Literal)>,
    selection: Vec<Selection>,
}

#[derive(Debug)]
enum Selection {
    Field(Field),
    Fragment {
        on: Option<String>,
        selection: Vec<Selection>,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Punct(char),
    Spread,
    Name(String),
    Int(String),
    Float(f64),
    Str(String),
}

fn tokens(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let character = chars[at];
        match character {
            ' ' | '\t' | '\n' | '\r' | ',' | '\u{feff}' => at += 1,
            '#' => {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            }
            '.' => {
                if chars.get(at..at + 3) != Some(&['.', '.', '.']) {
                    return Err("unexpected '.' in the query".to_string());
                }
                out.push(Token::Spread);
                at += 3;
            }
            '{' | '}' | '(' | ')' | ':' | '[' | ']' | '!' | '$' | '=' | '@' => {
                out.push(Token::Punct(character));
                at += 1;
            }
            '"' => {
                if chars.get(at..at + 3) == Some(&['"', '"', '"']) {
                    return Err("block strings are not supported".to_string());
                }
                at += 1;
                let mut text = String::new();
                loop {
                    let Some(&next) = chars.get(at) else {
                        return Err("unterminated string in the query".to_string());
                    };
                    at += 1;
                    match next {
                        '"' => break,
                        '\\' => {
                            let escaped = chars.get(at).copied().unwrap_or('\\');
                            at += 1;
                            match escaped {
                                'n' => text.push('\n'),
                                't' => text.push('\t'),
                                'r' => text.push('\r'),
                                'b' => text.push('\u{8}'),
                                'f' => text.push('\u{c}'),
                                'u' => {
                                    let digits: String =
                                        chars.get(at..at + 4).unwrap_or(&[]).iter().collect();
                                    let point = u32::from_str_radix(&digits, 16)
                                        .ok()
                                        .and_then(char::from_u32)
                                        .ok_or("invalid \\u escape in the query")?;
                                    text.push(point);
                                    at += 4;
                                }
                                other => text.push(other),
                            }
                        }
                        other => text.push(other),
                    }
                }
                out.push(Token::Str(text));
            }
            '-' | '0'..='9' => {
                let start = at;
                at += 1;
                while at < chars.len()
                    && (chars[at].is_ascii_alphanumeric() || ".+-".contains(chars[at]))
                {
                    at += 1;
                }
                let text: String = chars[start..at].iter().collect();
                if text
                    .bytes()
                    .all(|byte| byte == b'-' || byte.is_ascii_digit())
                {
                    out.push(Token::Int(text));
                } else {
                    out.push(Token::Float(
                        text.parse()
                            .map_err(|_| format!("invalid number {text} in the query"))?,
                    ));
                }
            }
            _ if character == '_' || character.is_ascii_alphabetic() => {
                let start = at;
                while at < chars.len() && (chars[at] == '_' || chars[at].is_ascii_alphanumeric()) {
                    at += 1;
                }
                out.push(Token::Name(chars[start..at].iter().collect()));
            }
            other => return Err(format!("unexpected {other:?} in the query")),
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Result<Token, String> {
        let token = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or("the query ends early")?;
        self.at += 1;
        Ok(token)
    }

    fn eat(&mut self, punct: char) -> bool {
        if self.peek() == Some(&Token::Punct(punct)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, punct: char) -> Result<(), String> {
        if self.eat(punct) {
            Ok(())
        } else {
            Err(format!("expected '{punct}' in the query"))
        }
    }

    fn name(&mut self) -> Result<String, String> {
        match self.next()? {
            Token::Name(name) => Ok(name),
            other => Err(format!("expected a name in the query, found {other:?}")),
        }
    }

    fn document(&mut self) -> Result<Vec<Selection>, String> {
        if let Some(Token::Name(keyword)) = self.peek().cloned() {
            match keyword.as_str() {
                "query" => {
                    self.at += 1;
                    if matches!(self.peek(), Some(Token::Name(_))) {
                        self.at += 1;
                    }
                    if self.eat('(') {
                        self.variable_definitions()?;
                    }
                }
                "mutation" | "subscription" => {
                    return Err(format!(
                        "a {keyword} writes to or watches the forge; the frozen forge is read-only"
                    ));
                }
                "fragment" => return Err("fragment definitions are not supported".to_string()),
                _ => return Err(format!("unexpected {keyword} at the start of the query")),
            }
        }
        let selection = self.selection_set()?;
        if self.at != self.tokens.len() {
            return Err(
                "the query holds more than one operation; only one is supported".to_string(),
            );
        }
        Ok(selection)
    }

    fn variable_definitions(&mut self) -> Result<(), String> {
        while !self.eat(')') {
            self.expect('$')?;
            self.name()?;
            self.expect(':')?;
            self.type_reference()?;
            if self.eat('=') {
                self.literal()?;
            }
        }
        Ok(())
    }

    fn type_reference(&mut self) -> Result<(), String> {
        if self.eat('[') {
            self.type_reference()?;
            self.expect(']')?;
        } else {
            self.name()?;
        }
        self.eat('!');
        Ok(())
    }

    fn literal(&mut self) -> Result<Literal, String> {
        Ok(match self.next()? {
            Token::Punct('$') => Literal::Variable(self.name()?),
            Token::Int(digits) => Literal::Int(digits),
            Token::Float(number) => Literal::Float(number),
            Token::Str(text) => Literal::Str(text),
            Token::Name(name) => match name.as_str() {
                "true" => Literal::Bool(true),
                "false" => Literal::Bool(false),
                "null" => Literal::Null,
                _ => Literal::Enum(name),
            },
            Token::Punct('[') => {
                let mut items = Vec::new();
                while !self.eat(']') {
                    items.push(self.literal()?);
                }
                Literal::List(items)
            }
            other => return Err(format!("unsupported argument value {other:?} in the query")),
        })
    }

    fn selection_set(&mut self) -> Result<Vec<Selection>, String> {
        self.expect('{')?;
        let mut selection = Vec::new();
        while !self.eat('}') {
            if self.peek() == Some(&Token::Spread) {
                self.at += 1;
                let on = match self.peek() {
                    Some(Token::Name(word)) if word == "on" => {
                        self.at += 1;
                        Some(self.name()?)
                    }
                    Some(Token::Name(_)) => {
                        return Err("named fragment spreads are not supported".to_string());
                    }
                    _ => None,
                };
                self.no_directive()?;
                selection.push(Selection::Fragment {
                    on,
                    selection: self.selection_set()?,
                });
                continue;
            }
            let first = self.name()?;
            let (key, name) = if self.eat(':') {
                (first, self.name()?)
            } else {
                (first.clone(), first)
            };
            let mut arguments = Vec::new();
            if self.eat('(') {
                while !self.eat(')') {
                    let argument = self.name()?;
                    self.expect(':')?;
                    arguments.push((argument, self.literal()?));
                }
            }
            self.no_directive()?;
            let selection_below = if self.peek() == Some(&Token::Punct('{')) {
                self.selection_set()?
            } else {
                Vec::new()
            };
            selection.push(Selection::Field(Field {
                key,
                name,
                arguments,
                selection: selection_below,
            }));
        }
        Ok(selection)
    }

    fn no_directive(&self) -> Result<(), String> {
        if self.peek() == Some(&Token::Punct('@')) {
            return Err("directives are not supported".to_string());
        }
        Ok(())
    }
}

/// The node types of a field's records, so inline fragments and `__typename` resolve.
pub(crate) fn child_type(parent: Option<&str>, field: &str) -> Option<&'static str> {
    Some(match field {
        "repository" | "baseRepository" | "headRepository" => "Repository",
        "pullRequest" => "PullRequest",
        "issue" | "closingIssuesReferences" => "Issue",
        "reviewThreads" => "PullRequestReviewThread",
        "reviews" | "pullRequestReview" | "review" => "PullRequestReview",
        "comments" if parent == Some("PullRequestReviewThread") => "PullRequestReviewComment",
        "comments" => "IssueComment",
        "replyTo" => "PullRequestReviewComment",
        "commits" => "PullRequestCommit",
        "commit" | "originalCommit" | "beforeCommit" | "afterCommit" => "Commit",
        "labels" | "label" => "Label",
        "author" | "user" => "User",
        "headRepositoryOwner" => "RepositoryOwner",
        "checkSuites" => "CheckSuite",
        "checkRuns" => "CheckRun",
        "status" => "Status",
        "contexts" => "StatusContext",
        "app" => "App",
        "workflowRun" => "WorkflowRun",
        "workflow" => "Workflow",
        _ => return None,
    })
}

fn pascal(item_type: &str) -> String {
    item_type
        .split('_')
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            let mut chars = lower.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

struct Evaluation<'a> {
    capture: &'a Capture,
    variables: &'a Object,
    errors: Vec<String>,
}

enum Scope<'a> {
    Root,
    Repository { name: String, main: bool },
    Record(&'a Value, Option<&'static str>),
}

impl<'a> Evaluation<'a> {
    fn value(&self, literal: &Literal) -> Result<Value, String> {
        Ok(match literal {
            Literal::Variable(name) => self.variables.get(name).cloned().unwrap_or(Value::Null),
            Literal::Int(digits) => Value::Int(digits.clone()),
            Literal::Float(number) => Value::Float(*number),
            Literal::Str(text) | Literal::Enum(text) => Value::Str(text.clone()),
            Literal::Bool(flag) => Value::Bool(*flag),
            Literal::Null => Value::Null,
            Literal::List(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.value(item))
                    .collect::<Result<_, _>>()?,
            ),
        })
    }

    fn arguments(&self, field: &Field) -> Result<Object, String> {
        field
            .arguments
            .iter()
            .map(|(name, literal)| Ok((name.clone(), self.value(literal)?)))
            .collect()
    }

    fn object(&mut self, scope: &Scope<'a>, selection: &[Selection], path: &str) -> Value {
        let mut out = Object::default();
        self.fill(scope, selection, path, &mut out);
        Value::Object(out)
    }

    fn fill(&mut self, scope: &Scope<'a>, selection: &[Selection], path: &str, out: &mut Object) {
        for item in selection {
            match item {
                Selection::Fragment { on, selection } => {
                    let applies = match (on, scope_type(scope)) {
                        (None, _) | (_, None) => true,
                        (Some(on), Some(actual)) => on == actual,
                    };
                    if applies {
                        self.fill(scope, selection, path, out);
                    }
                }
                Selection::Field(field) => {
                    let place = format!("{path}.{}", field.key);
                    let value = match self.field(scope, field, &place) {
                        Ok(value) => value,
                        Err(message) => {
                            self.errors.push(format!("{message} ({})", &place[1..]));
                            Value::Null
                        }
                    };
                    out.insert(field.key.clone(), value);
                }
            }
        }
    }

    fn field(&mut self, scope: &Scope<'a>, field: &Field, path: &str) -> Result<Value, String> {
        let arguments = self.arguments(field)?;
        if field.name == "__typename" {
            return scope_type(scope)
                .map(|name| Value::Str(name.to_string()))
                .ok_or_else(|| "__typename is not in the frozen capture".to_string());
        }
        match scope {
            Scope::Root => self.root(field, &arguments, path),
            Scope::Repository { name, main } => {
                self.repository(name, *main, field, &arguments, path)
            }
            Scope::Record(record, kind) => self.record(record, *kind, field, &arguments, path),
        }
    }

    fn root(&mut self, field: &Field, arguments: &Object, path: &str) -> Result<Value, String> {
        match field.name.as_str() {
            "repository" => {
                let owner = text_argument(arguments, "owner")?;
                let name = text_argument(arguments, "name")?;
                let slug = format!("{owner}/{name}");
                let main = slug.eq_ignore_ascii_case(&self.capture.repository);
                if !main && !self.capture.has_issue_repository(&slug) {
                    return Err(format!(
                        "Could not resolve to a Repository with the name '{slug}'."
                    ));
                }
                let scope = Scope::Repository {
                    name: if main {
                        self.capture.repository.clone()
                    } else {
                        slug
                    },
                    main,
                };
                Ok(self.object(&scope, &field.selection, path))
            }
            "node" => {
                let id = text_argument(arguments, "id")?;
                let capture = self.capture;
                match capture.node(&id) {
                    Some((record, kind)) => {
                        Ok(self.object(&Scope::Record(record, kind), &field.selection, path))
                    }
                    None => Err(format!(
                        "Could not resolve to a node with the global id of '{id}'"
                    )),
                }
            }
            other => Err(format!(
                "the frozen forge does not answer the query field {other}"
            )),
        }
    }

    fn repository(
        &mut self,
        slug: &str,
        main: bool,
        field: &Field,
        arguments: &Object,
        path: &str,
    ) -> Result<Value, String> {
        let capture = self.capture;
        match field.name.as_str() {
            "nameWithOwner" => Ok(Value::Str(slug.to_string())),
            "name" => Ok(Value::Str(
                slug.split_once('/')
                    .map_or(slug, |(_, name)| name)
                    .to_string(),
            )),
            "url" => Ok(Value::Str(format!("https://github.com/{slug}"))),
            "owner" => {
                let login = slug.split_once('/').map_or(slug, |(owner, _)| owner);
                let mut owner = Object::default();
                for item in &field.selection {
                    let Selection::Field(inner) = item else {
                        return Err("fragments on a repository owner are not supported".to_string());
                    };
                    match inner.name.as_str() {
                        "login" => owner.insert(inner.key.clone(), Value::Str(login.to_string())),
                        other => {
                            return Err(format!(
                                "{other} of a repository owner is not in the frozen capture"
                            ));
                        }
                    }
                }
                Ok(Value::Object(owner))
            }
            "pullRequest" if main => {
                let number = int_argument(arguments, "number")?;
                if number != capture.number {
                    return Err(format!(
                        "Could not resolve to a PullRequest with the number of {number}."
                    ));
                }
                Ok(self.object(
                    &Scope::Record(&capture.pull, Some("PullRequest")),
                    &field.selection,
                    path,
                ))
            }
            "issue" => {
                let number = int_argument(arguments, "number")?;
                match capture.issue(slug, number) {
                    Some(issue) => Ok(self.object(
                        &Scope::Record(issue, Some("Issue")),
                        &field.selection,
                        path,
                    )),
                    None => Err(format!(
                        "Could not resolve to an Issue with the number of {number}."
                    )),
                }
            }
            other => match capture
                .repository_record
                .as_object()
                .and_then(|record| record.get(other))
            {
                Some(value) if main && field.selection.is_empty() => Ok(value.clone()),
                _ => Err(format!(
                    "{other} of a repository is not in the frozen capture"
                )),
            },
        }
    }

    fn record(
        &mut self,
        record: &'a Value,
        kind: Option<&'static str>,
        field: &Field,
        arguments: &Object,
        path: &str,
    ) -> Result<Value, String> {
        let Some(found) = record
            .as_object()
            .and_then(|object| object.get(&field.name))
        else {
            if UNKNOWN_AT_CUTOFF.contains(&field.name.as_str()) {
                return Ok(Value::Null);
            }
            return Err(format!(
                "{} of {} is not in the frozen capture",
                field.name,
                kind.unwrap_or("this record")
            ));
        };
        let child = child_type(kind, &field.name);
        if let Some(nodes) = found
            .as_object()
            .filter(|connection| connection.get("nodes").is_some())
            .and_then(|connection| connection.get("nodes"))
            .and_then(Value::as_array)
        {
            return self.connection(nodes, kind, child, field, arguments, path);
        }
        if !arguments.is_empty() {
            return Err(format!(
                "arguments to {} are not supported by the frozen forge",
                field.name
            ));
        }
        match found {
            Value::Object(_) if !field.selection.is_empty() => {
                Ok(self.object(&Scope::Record(found, child), &field.selection, path))
            }
            Value::Array(items) if !field.selection.is_empty() => Ok(Value::Array(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        self.object(
                            &Scope::Record(item, child),
                            &field.selection,
                            &format!("{path}.{index}"),
                        )
                    })
                    .collect(),
            )),
            Value::Object(_) | Value::Array(_) => {
                Err(format!("{} needs a selection of subfields", field.name))
            }
            Value::Null => Ok(Value::Null),
            scalar if field.selection.is_empty() => Ok(scalar.clone()),
            _ => Err(format!("{} has no subfields", field.name)),
        }
    }

    fn connection(
        &mut self,
        nodes: &'a [Value],
        parent: Option<&'static str>,
        child: Option<&'static str>,
        field: &Field,
        arguments: &Object,
        path: &str,
    ) -> Result<Value, String> {
        for name in arguments.iter().map(|(name, _)| name.as_str()) {
            if !matches!(name, "first" | "last" | "after" | "before" | "itemTypes") {
                return Err(format!(
                    "the argument {name} of {} is not supported by the frozen forge",
                    field.name
                ));
            }
        }
        let mut selected: Vec<(usize, &'a Value)> = nodes.iter().enumerate().collect();
        if let Some(types) = arguments
            .get("itemTypes")
            .filter(|types| !matches!(types, Value::Null))
        {
            let captured: &[&str] = if parent == Some("PullRequest") {
                &PULL_TIMELINE
            } else {
                &ISSUE_TIMELINE
            };
            let wanted: Vec<String> = types
                .as_array()
                .ok_or("itemTypes must be a list")?
                .iter()
                .map(Value::py_str)
                .collect();
            if let Some(missing) = wanted
                .iter()
                .find(|kind| !captured.contains(&kind.as_str()))
            {
                return Err(format!(
                    "timeline items of type {missing} are not in the frozen capture"
                ));
            }
            let names: Vec<String> = wanted.iter().map(|kind| pascal(kind)).collect();
            selected.retain(|(_, node)| {
                node.as_object()
                    .and_then(|node| node.get("__typename"))
                    .and_then(Value::as_str)
                    .is_some_and(|name| names.iter().any(|wanted| wanted == name))
            });
        } else if field.name == "timelineItems" {
            return Err("timelineItems needs itemTypes in the frozen forge".to_string());
        }
        let total = selected.len();
        let after = cursor_argument(arguments, "after")?;
        let before = cursor_argument(arguments, "before")?;
        let mut window: Vec<(usize, &'a Value)> = selected
            .iter()
            .enumerate()
            .filter(|(position, _)| after.is_none_or(|after| *position > after))
            .filter(|(position, _)| before.is_none_or(|before| *position < before))
            .map(|(position, (_, node))| (position, *node))
            .collect();
        let first = count_argument(arguments, "first")?;
        let last = count_argument(arguments, "last")?;
        if first.is_none() && last.is_none() {
            return Err(format!(
                "You must provide a `first` or `last` value to properly paginate the `{}` connection.",
                field.name
            ));
        }
        let (mut more_after, mut more_before) =
            (before.is_some_and(|before| before < total), after.is_some());
        if let Some(first) = first
            && window.len() > first
        {
            window.truncate(first);
            more_after = true;
        }
        if let Some(last) = last
            && window.len() > last
        {
            window.drain(..window.len() - last);
            more_before = true;
        }
        let mut out = Object::default();
        for item in &field.selection {
            let Selection::Field(inner) = item else {
                return Err(format!(
                    "fragments on the {} connection are not supported",
                    field.name
                ));
            };
            let place = format!("{path}.{}", inner.key);
            let value = match inner.name.as_str() {
                "totalCount" => Value::Int(total.to_string()),
                "nodes" => Value::Array(
                    window
                        .iter()
                        .map(|(position, node)| {
                            let kind = node_type(node).or(child);
                            self.object(
                                &Scope::Record(node, kind),
                                &inner.selection,
                                &format!("{place}.{position}"),
                            )
                        })
                        .collect(),
                ),
                "edges" => Value::Array(
                    window
                        .iter()
                        .map(|(position, node)| {
                            let mut edge = Object::default();
                            for part in &inner.selection {
                                let Selection::Field(part) = part else {
                                    continue;
                                };
                                let value = match part.name.as_str() {
                                    "cursor" => Value::Str(cursor(*position)),
                                    "node" => {
                                        let kind = node_type(node).or(child);
                                        self.object(
                                            &Scope::Record(node, kind),
                                            &part.selection,
                                            &place,
                                        )
                                    }
                                    other => {
                                        self.errors.push(format!(
                                            "{other} of an edge is not supported ({})",
                                            &place[1..]
                                        ));
                                        Value::Null
                                    }
                                };
                                edge.insert(part.key.clone(), value);
                            }
                            Value::Object(edge)
                        })
                        .collect(),
                ),
                "pageInfo" => {
                    let mut info = Object::default();
                    for part in &inner.selection {
                        let Selection::Field(part) = part else {
                            continue;
                        };
                        let value = match part.name.as_str() {
                            "hasNextPage" => Value::Bool(more_after),
                            "hasPreviousPage" => Value::Bool(more_before),
                            "startCursor" => window
                                .first()
                                .map_or(Value::Null, |(position, _)| Value::Str(cursor(*position))),
                            "endCursor" => window
                                .last()
                                .map_or(Value::Null, |(position, _)| Value::Str(cursor(*position))),
                            other => {
                                self.errors.push(format!(
                                    "{other} of pageInfo is not supported ({})",
                                    &place[1..]
                                ));
                                Value::Null
                            }
                        };
                        info.insert(part.key.clone(), value);
                    }
                    Value::Object(info)
                }
                other => {
                    self.errors.push(format!(
                        "{other} of the {} connection is not supported ({})",
                        field.name,
                        &place[1..]
                    ));
                    Value::Null
                }
            };
            out.insert(inner.key.clone(), value);
        }
        Ok(Value::Object(out))
    }
}

fn node_type(node: &Value) -> Option<&'static str> {
    let name = node.as_object()?.get("__typename")?.as_str()?;
    [
        "HeadRefForcePushedEvent",
        "RenamedTitleEvent",
        "ReadyForReviewEvent",
        "ConvertToDraftEvent",
        "LabeledEvent",
        "UnlabeledEvent",
        "BaseRefChangedEvent",
        "ReviewDismissedEvent",
        "ClosedEvent",
        "ReopenedEvent",
    ]
    .into_iter()
    .find(|known| *known == name)
}

fn scope_type(scope: &Scope<'_>) -> Option<&'static str> {
    match scope {
        Scope::Root => Some("Query"),
        Scope::Repository { .. } => Some("Repository"),
        Scope::Record(record, kind) => node_type(record).or(*kind),
    }
}

fn cursor(position: usize) -> String {
    format!("frozen:{position}")
}

fn cursor_argument(arguments: &Object, name: &str) -> Result<Option<usize>, String> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Str(text)) => text
            .strip_prefix("frozen:")
            .and_then(|digits| digits.parse().ok())
            .map(Some)
            .ok_or_else(|| format!("`{text}` does not appear to be a valid cursor.")),
        Some(_) => Err(format!("{name} must be a cursor string")),
    }
}

fn count_argument(arguments: &Object, name: &str) -> Result<Option<usize>, String> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Int(digits)) => {
            let count: usize = digits
                .parse()
                .map_err(|_| format!("{name} must be a non-negative integer"))?;
            if count > PAGE_LIMIT {
                return Err(format!(
                    "Requesting {count} records exceeds the `{name}` limit of {PAGE_LIMIT} records."
                ));
            }
            Ok(Some(count))
        }
        Some(_) => Err(format!("{name} must be an integer")),
    }
}

fn text_argument(arguments: &Object, name: &str) -> Result<String, String> {
    match arguments.get(name) {
        Some(Value::Str(text)) => Ok(text.clone()),
        Some(Value::Int(digits)) => Ok(digits.clone()),
        _ => Err(format!("the argument {name} is missing")),
    }
}

fn int_argument(arguments: &Object, name: &str) -> Result<u64, String> {
    match arguments.get(name) {
        Some(Value::Int(digits)) => digits
            .parse()
            .map_err(|_| format!("{name} must be a number")),
        Some(Value::Str(text)) => text.parse().map_err(|_| format!("{name} must be a number")),
        _ => Err(format!("the argument {name} is missing")),
    }
}

/// The response GitHub would return for `query` with `variables`: `data`, then `errors` when any field failed.
pub fn answer(capture: &Capture, query: &str, variables: &Object) -> Result<(Value, bool), String> {
    let selection = Parser {
        tokens: tokens(query)?,
        at: 0,
    }
    .document()?;
    let mut evaluation = Evaluation {
        capture,
        variables,
        errors: Vec::new(),
    };
    let data = evaluation.object(&Scope::Root, &selection, "");
    let mut response = Object::default();
    response.insert("data".to_string(), data);
    let failed = !evaluation.errors.is_empty();
    if failed {
        response.insert(
            "errors".to_string(),
            Value::Array(
                evaluation
                    .errors
                    .into_iter()
                    .map(|message| {
                        Value::Object(Object::from_iter([(
                            "message".to_string(),
                            Value::Str(message),
                        )]))
                    })
                    .collect(),
            ),
        );
    }
    Ok((Value::Object(response), failed))
}
