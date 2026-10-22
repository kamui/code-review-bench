use super::{Capture, graphql, read_json};
use crate::pyjson::{Object, Value};
use std::path::Path;

fn capture(target: &str) -> Capture {
    let forge = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/targets")
        .join(target)
        .join("forge");
    Capture::load(
        &read_json(&forge.join("graphql.json")).expect("graphql.json"),
        &read_json(&forge.join("issues.json")).expect("issues.json"),
    )
    .expect("capture")
}

fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    path.split('.')
        .fold(value, |value, key| match key.parse::<usize>() {
            Ok(index) => &value.as_array().expect("array")[index],
            Err(_) => value
                .as_object()
                .and_then(|object| object.get(key))
                .unwrap_or_else(|| panic!("no {key} in {}", value.py_repr())),
        })
}

fn ask(capture: &Capture, query: &str, variables: &[(&str, Value)]) -> (Value, bool) {
    let variables: Object = variables
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect();
    graphql::answer(capture, query, &variables).expect("answer")
}

fn errors(response: &Value) -> Vec<String> {
    at(response, "errors")
        .as_array()
        .expect("errors")
        .iter()
        .map(|error| at(error, "message").py_str())
        .collect()
}

#[test]
fn answers_the_review_code_root_query_with_unknowns_as_null() {
    let capture = capture("l-bokeh-9232");
    let query = "query($owner:String!,$name:String!,$number:Int!){ repository(owner:$owner,name:$name){ url pullRequest(number:$number){ title state merged updatedAt baseRepository{ url } reviewThreads(first:100){ totalCount pageInfo{ hasNextPage endCursor } nodes{ id isResolved comments(first:100){ nodes{ fullDatabaseId author{login} replyTo{ fullDatabaseId } } } } } closingIssuesReferences(first:20){ nodes{ number comments(first:100){ totalCount } } } } } }";
    let (response, failed) = ask(
        &capture,
        query,
        &[
            ("owner", Value::Str("bokeh".into())),
            ("name", Value::Str("bokeh".into())),
            ("number", Value::Int("9232".into())),
        ],
    );
    assert!(!failed, "{}", response.py_repr());
    let pull = at(&response, "data.repository.pullRequest");
    assert_eq!(
        at(&response, "data.repository.url").py_str(),
        "https://github.com/bokeh/bokeh"
    );
    assert_eq!(at(pull, "state").py_str(), "OPEN");
    assert!(matches!(at(pull, "merged"), Value::Bool(false)));
    assert!(matches!(at(pull, "updatedAt"), Value::Null));
    assert!(matches!(
        at(pull, "reviewThreads.nodes.0.isResolved"),
        Value::Null
    ));
    assert!(matches!(
        at(pull, "reviewThreads.pageInfo.hasNextPage"),
        Value::Bool(false)
    ));
    assert_eq!(
        at(pull, "reviewThreads.pageInfo.endCursor").py_str(),
        "frozen:0"
    );
    assert_eq!(
        at(pull, "closingIssuesReferences.nodes.0.number").py_str(),
        "9129"
    );
    assert!(matches!(
        at(pull, "reviewThreads.nodes.0.comments.nodes.0.replyTo"),
        Value::Null
    ));
    assert_eq!(
        at(
            pull,
            "reviewThreads.nodes.0.comments.nodes.1.replyTo.fullDatabaseId"
        )
        .py_str(),
        at(
            pull,
            "reviewThreads.nodes.0.comments.nodes.0.fullDatabaseId"
        )
        .py_str()
    );
}

#[test]
fn pages_a_connection_by_its_cursors() {
    let capture = capture("m-grpc-go-7390");
    let page = |after: Value| {
        ask(
            &capture,
            "query($after:String){ repository(owner:\"grpc\",name:\"grpc-go\"){ pullRequest(number:7390){ reviews(first:10,after:$after){ totalCount pageInfo{ hasNextPage endCursor } nodes{ fullDatabaseId } } } } }",
            &[("after", after)],
        )
    };
    let (first, failed) = page(Value::Null);
    assert!(!failed);
    let reviews = at(&first, "data.repository.pullRequest.reviews");
    assert_eq!(at(reviews, "totalCount").py_str(), "15");
    assert_eq!(at(reviews, "nodes").as_array().expect("nodes").len(), 10);
    assert!(matches!(
        at(reviews, "pageInfo.hasNextPage"),
        Value::Bool(true)
    ));
    let (second, failed) = page(at(reviews, "pageInfo.endCursor").clone());
    assert!(!failed);
    let rest = at(&second, "data.repository.pullRequest.reviews");
    assert_eq!(at(rest, "nodes").as_array().expect("nodes").len(), 5);
    assert!(matches!(
        at(rest, "pageInfo.hasNextPage"),
        Value::Bool(false)
    ));
    let (last, _) = ask(
        &capture,
        "{ repository(owner:\"grpc\",name:\"grpc-go\"){ pullRequest(number:7390){ commits(last:1){ nodes{ commit{ oid checkSuites(first:5){ totalCount } } } } } } }",
        &[],
    );
    let commit = at(&last, "data.repository.pullRequest.commits.nodes.0.commit");
    assert_eq!(
        at(commit, "oid").py_str(),
        at(&capture.pull, "headRefOid").py_str()
    );
    assert!(matches!(
        at(commit, "checkSuites.totalCount"),
        Value::Int(_)
    ));
}

#[test]
fn resolves_a_thread_node_and_filters_timeline_types() {
    let capture = capture("l-bokeh-9232");
    let thread = at(&capture.pull, "reviewThreads.nodes.0.id").py_str();
    let (response, failed) = ask(
        &capture,
        "query($thread:ID!){ node(id:$thread){ __typename ... on PullRequestReviewThread { id comments(first:1){ totalCount nodes{ body } } } ... on Issue { title } } }",
        &[("thread", Value::Str(thread.clone()))],
    );
    assert!(!failed, "{}", response.py_repr());
    assert_eq!(
        at(&response, "data.node.__typename").py_str(),
        "PullRequestReviewThread"
    );
    assert_eq!(at(&response, "data.node.id").py_str(), thread);
    assert!(
        at(&response, "data.node")
            .as_object()
            .expect("node")
            .get("title")
            .is_none()
    );
    let (response, failed) = ask(
        &capture,
        "{ repository(owner:\"bokeh\",name:\"bokeh\"){ pullRequest(number:9232){ timelineItems(first:50,itemTypes:[LABELED_EVENT]){ nodes{ __typename } } } } }",
        &[],
    );
    assert!(!failed);
    for node in at(&response, "data.repository.pullRequest.timelineItems.nodes")
        .as_array()
        .expect("nodes")
    {
        assert_eq!(at(node, "__typename").py_str(), "LabeledEvent");
    }
    let (response, failed) = ask(
        &capture,
        "{ repository(owner:\"bokeh\",name:\"bokeh\"){ pullRequest(number:9232){ timelineItems(first:50,itemTypes:[MERGED_EVENT]){ totalCount } } } }",
        &[],
    );
    assert!(failed);
    assert!(errors(&response)[0].contains("MERGED_EVENT are not in the frozen capture"));
}

#[test]
fn refuses_what_the_capture_does_not_hold() {
    let capture = capture("l-bokeh-9232");
    let (response, failed) = ask(
        &capture,
        "{ repository(owner:\"bokeh\",name:\"bokeh\"){ pullRequest(number:9232){ title mergeable } } }",
        &[],
    );
    assert!(failed);
    assert_eq!(
        at(&response, "data.repository.pullRequest.title").py_str(),
        at(&capture.pull, "title").py_str()
    );
    assert_eq!(
        errors(&response),
        [
            "mergeable of PullRequest is not in the frozen capture (repository.pullRequest.mergeable)"
        ]
    );
    for (query, needle) in [
        (
            "{ repository(owner:\"bokeh\",name:\"bokeh\"){ pullRequest(number:1){ title } } }",
            "number of 1",
        ),
        (
            "{ repository(owner:\"other\",name:\"repo\"){ url } }",
            "'other/repo'",
        ),
        (
            "{ repository(owner:\"bokeh\",name:\"bokeh\"){ pullRequest(number:9232){ reviews{ totalCount } } } }",
            "`first` or `last`",
        ),
        (
            "{ viewer{ login } }",
            "does not answer the query field viewer",
        ),
    ] {
        let (response, failed) = ask(&capture, query, &[]);
        assert!(failed, "{query}");
        assert!(
            errors(&response)[0].contains(needle),
            "{:?}",
            errors(&response)
        );
    }
    let refused = graphql::answer(
        &capture,
        "mutation{ addComment(input:{}){ clientMutationId } }",
        &Object::default(),
    );
    assert!(refused.is_err_and(|error| error.contains("read-only")));
}

#[test]
fn serves_an_originating_issue_from_issues_json() {
    let capture = capture("j-trpc-5017");
    let (response, failed) = ask(
        &capture,
        "query($issue:Int!){ repository(owner:\"trpc\",name:\"trpc\"){ issue(number:$issue){ number title updatedAt comments(first:100){ totalCount } } } }",
        &[("issue", Value::Int("5020".into()))],
    );
    assert!(!failed, "{}", response.py_repr());
    assert_eq!(
        at(&response, "data.repository.issue.number").py_str(),
        "5020"
    );
    assert!(matches!(
        at(&response, "data.repository.issue.updatedAt"),
        Value::Null
    ));
}
