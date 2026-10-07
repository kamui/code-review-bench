#!/usr/bin/env python3
"""Exercise the phase-1 packet builder through the build_packet.py CLI.

Usage: python3 bench/tools/test_build_packet.py
Inputs: the saved GraphQL response in test_build_packet_fixture.json, small saved REST bodies
for the two optional reference fetches, and a temporary git mirror built per test. No network:
every forge call is replayed through --replay.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("build_packet.py")
FIXTURE = Path(__file__).with_name("test_build_packet_fixture.json")

CUTOFF = "2026-03-10T12:00:00Z"
PUSHED = "2026-03-09T07:05:00Z"
OPENED = "2026-03-07T07:30:00Z"

# a double quote forces git to quote the path whatever core.quotePath is set to, and is ASCII, so
# the fixture does not depend on how the filesystem normalises non-ASCII names
QUOTED_DIRECTORY = 'docs/q"dir'


REF_PR = {
    "title": "Retry ceiling, first attempt",
    "body": "Superseded by the reviewed pull request.",
    "created_at": "2026-02-20T09:00:00Z",
    "updated_at": "2026-02-20T09:00:00Z",
    "comments": 2,
    "user": {"login": "carol"},
    "state": "closed",
    "merged": False,
    "closed_at": "2026-03-10T12:00:05Z",
}
REF_PR_COMMENTS = [
    {"user": {"login": "alice"}, "created_at": "2026-02-21T09:00:00Z", "updated_at": "2026-02-21T09:00:00Z", "body": "Closing in favour of the bounded loop."},
    {"user": {"login": "dana"}, "created_at": "2026-03-16T09:00:00Z", "updated_at": "2026-03-16T09:00:00Z", "body": "Post-merge note that must not reach the packet."},
]
SPEC_ISSUE = {
    "html_url": "https://github.com/other/spec/issues/12",
    "title": "Reconnect must give up",
    "body": "The client must stop retrying after a bounded number of attempts.",
    "created_at": "2026-02-10T09:00:00Z",
    "updated_at": "2026-02-10T09:00:00Z",
    "comments": 2,
    "user": {"login": "erin"},
}
SPEC_ISSUE_COMMENTS = [
    {"user": {"login": "erin"}, "created_at": "2026-02-11T09:00:00Z", "updated_at": "2026-02-11T09:00:00Z", "body": "A ceiling of five attempts is enough."},
    {"user": {"login": "dana"}, "created_at": "2026-03-17T09:00:00Z", "updated_at": "2026-03-17T09:00:00Z", "body": "Post-merge note that must not reach the packet."},
]


class BuildPacketTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.mirror = self.directory / "mirror"
        self.replay = self.directory / "replay"
        self.replay.mkdir()
        self.out = self.directory / "packets" / "a" / "packet.md"
        self.merge_base, self.head = self.build_mirror()
        self.fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
        self.fixture["data"]["repository"]["pullRequest"]["headRefOid"] = self.head

    # --- fixtures -------------------------------------------------------

    def git(self, *args: str, when: str = "2026-03-07T07:00:00+00:00") -> str:
        environment = {
            "PATH": "/usr/bin:/bin:/usr/local/bin",
            "HOME": str(self.directory),
            "GIT_AUTHOR_NAME": "Bob", "GIT_AUTHOR_EMAIL": "bob@example.com",
            "GIT_COMMITTER_NAME": "Bob", "GIT_COMMITTER_EMAIL": "bob@example.com",
            "GIT_AUTHOR_DATE": when, "GIT_COMMITTER_DATE": when,
        }
        done = subprocess.run(["git", "-C", str(self.mirror), *args], check=True,
                              capture_output=True, text=True, encoding="utf-8", env=environment)
        return done.stdout.strip()

    def build_mirror(self) -> tuple:
        """A two-commit repository standing in for the staging mirror."""
        self.mirror.mkdir()
        subprocess.run(["git", "init", "-q", "-b", "main", str(self.mirror)], check=True, capture_output=True)
        self.write(self.mirror / "AGENTS.md", "Root guidance.\n")
        self.write(self.mirror / "src" / "AGENTS.md", "Scoped guidance for src/.\n")
        self.write(self.mirror / "src" / "retry.rs", "fn retry() { loop {} }\n")
        self.write(self.mirror / "docs" / "notes.md", "Notes.\n")
        self.write(self.mirror / QUOTED_DIRECTORY / "AGENTS.md", "Scoped guidance for a quoted path.\n")
        self.write(self.mirror / QUOTED_DIRECTORY / "a.md", "A file git prints quoted.\n")
        self.write(self.mirror / "guide" / "AGENTS.md", "Scoped guidance for guide/.\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "Base of the reviewed change")
        base = self.git("rev-parse", "HEAD")
        self.write(self.mirror / "src" / "retry.rs", "fn retry() { for _ in 0..5 {} }\n")
        self.write(self.mirror / "src" / "lib.rs", "pub mod retry;\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "Bound the reconnect retry loop", when="2026-03-09T07:00:00+00:00")
        head = self.git("rev-parse", "HEAD")
        return base, head

    def touch_the_quoted_path(self) -> None:
        """Add a commit changing the file under the quoted directory, and repin the head to it."""
        self.write(self.mirror / QUOTED_DIRECTORY / "a.md", "A file git prints quoted, changed.\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "Touch the quoted path", when="2026-03-09T08:00:00+00:00")
        self.head = self.git("rev-parse", "HEAD")
        self.fixture["data"]["repository"]["pullRequest"]["headRefOid"] = self.head

    def rename_into_guide(self) -> None:
        """Add a commit renaming docs/notes.md into guide/, and repin the head to it."""
        self.git("mv", "docs/notes.md", "guide/notes.md")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "Move the notes under guide/", when="2026-03-09T08:00:00+00:00")
        self.head = self.git("rev-parse", "HEAD")
        self.fixture["data"]["repository"]["pullRequest"]["headRefOid"] = self.head

    @staticmethod
    def write(path: Path, text: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def drop_the_closing_reference(self) -> None:
        """Leave the pull request with no closing issue, connection count included."""
        references = self.fixture["data"]["repository"]["pullRequest"]["closingIssuesReferences"]
        references["nodes"], references["totalCount"] = [], 0

    def save_replay(self, **bodies) -> None:
        """Save the forge responses this run replays; the GraphQL body defaults to the fixture."""
        bodies.setdefault("graphql", self.fixture)
        for name, body in bodies.items():
            path = self.replay / (name.replace("_", "-") + ".json")
            path.write_text(json.dumps(body), encoding="utf-8")

    def run_cli(self, *options: str, save: bool = True, cutoff: str | None = CUTOFF) -> subprocess.CompletedProcess:
        """Run the builder at the merge instant, or with ``cutoff=None`` at the cutoff it works out."""
        if save:
            self.save_replay()
        arguments = [
            sys.executable, str(SCRIPT),
            "--repo", "example/retry", "--pr", "3952",
            "--head", self.head, "--merge-base", self.merge_base, "--base-sha", self.merge_base,
            "--staging", str(self.mirror), "--target", "a",
            "--replay", str(self.replay), "--out", str(self.out),
        ]
        if cutoff:
            arguments += ["--cutoff", cutoff]
        return subprocess.run([*arguments, *options], capture_output=True, text=True, encoding="utf-8")

    def build_ok(self, *options: str, save: bool = True, cutoff: str | None = CUTOFF) -> str:
        result = self.run_cli(*options, save=save, cutoff=cutoff)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return self.out.read_text(encoding="utf-8")

    def pull(self) -> dict:
        return self.fixture["data"]["repository"]["pullRequest"]

    def date_the_timeline(self, *events: dict) -> None:
        self.pull()["timelineItems"] = {"pageInfo": {"hasNextPage": False}, "nodes": list(events)}

    def force_push(self, when: str = PUSHED, oid: str | None = None) -> dict:
        return {"__typename": "HeadRefForcePushedEvent", "createdAt": when, "afterCommit": {"oid": oid or self.head}}

    def check_the_head(self, *instants: str) -> None:
        self.pull()["headCommit"] = {"nodes": [{"commit": {"oid": self.head, "checkSuites": {
            "totalCount": len(instants), "nodes": [{"createdAt": instant} for instant in instants]}}}]}

    def open_with_the_only_commit(self, merged_at: str = "2026-03-06T09:00:00Z") -> dict:
        """One commit whose parent an earlier pull request merged into main; returns that parent."""
        parent = {"oid": "9" * 40, "associatedPullRequests": {"nodes": [
            {"mergedAt": merged_at, "baseRefName": "main", "mergeCommit": {"oid": "9" * 40}}]}}
        self.check_the_head()
        self.pull()["headCommit"]["nodes"][0]["commit"]["parents"] = {"totalCount": 1, "nodes": [parent]}
        self.pull()["commits"]["totalCount"] = 1
        return parent

    def save_edits(self, **histories: list) -> None:
        """Save the edit histories of the named nodes: each a list of (editedAt, text) revisions."""
        self.save_replay(edits={"data": {"nodes": [
            {"id": identifier, "userContentEdits": {"totalCount": len(revisions), "nodes": [
                {"editedAt": when, "deletedAt": None, "diff": text} for when, text in revisions]}}
            for identifier, revisions in histories.items()]}})

    # --- the last push --------------------------------------------------

    def test_the_default_cutoff_is_the_force_push_that_made_the_head(self) -> None:
        self.date_the_timeline(self.force_push("2026-03-08T12:00:00Z", "0" * 40), self.force_push())
        result = self.run_cli(cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cutoff {PUSHED}; omitted after cutoff: "
                      "{'reviews': 2, 'thread_comments': 2, 'conversation': 1, 'issue_comments': 1}",
                      result.stdout)
        self.assertIn("cutoff source: the force-push event that made this commit the head", result.stdout)
        packet = self.out.read_text(encoding="utf-8")
        self.assertIn(f"## 6. Prior review state through the frozen cutoff `{PUSHED}` (the last push)", packet)
        self.assertIn("Bounded in the follow-up commit.", packet)
        self.assertNotIn("| APPROVED |", packet)

    def test_a_pull_request_opened_with_its_only_commit_cuts_at_the_opening(self) -> None:
        self.date_the_timeline()
        self.open_with_the_only_commit()
        result = self.run_cli(cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cutoff {OPENED};", result.stdout)
        self.assertIn("cutoff source: the pull request was opened with its only commit", result.stdout)
        packet = self.out.read_text(encoding="utf-8")
        self.assertIn(f"frozen cutoff `{OPENED}` (the pull request's opening)", packet)
        self.assertNotIn("Opening for review; the ceiling is configurable.", packet)

    def test_one_listed_commit_does_not_show_the_head_was_there_at_the_opening(self) -> None:
        def another_branch(parent: dict) -> None:
            parent["associatedPullRequests"]["nodes"][0]["baseRefName"] = "release"

        def not_the_merge_commit(parent: dict) -> None:
            parent["associatedPullRequests"]["nodes"][0]["mergeCommit"]["oid"] = "8" * 40

        def never_merged(parent: dict) -> None:
            parent["associatedPullRequests"]["nodes"] = []

        cases = {
            "force-pushed to an earlier head, then fast-forwarded": (lambda parent: None, [self.force_push(oid="0" * 40)]),
            "base changed": (lambda parent: None, [{"__typename": "BaseRefChangedEvent"}]),
            "base force-pushed": (lambda parent: None, [{"__typename": "BaseRefForcePushedEvent"}]),
            "parent merged into another branch": (another_branch, []),
            "parent is not that pull request's merge commit": (not_the_merge_commit, []),
            "parent never merged by a pull request": (never_merged, []),
        }
        for label, (change, events) in cases.items():
            with self.subTest(case=label):
                self.date_the_timeline(*events)
                change(self.open_with_the_only_commit())
                result = self.run_cli(cutoff=None)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("cannot establish the last push", result.stderr)
        with self.subTest(case="parent merged after the opening"):
            self.date_the_timeline()
            self.open_with_the_only_commit(merged_at="2026-03-07T07:30:01Z")
            self.assertEqual(self.run_cli(cutoff=None).returncode, 2)
        with self.subTest(case="a later check suite dates the fast-forward"):
            self.date_the_timeline(self.force_push(oid="0" * 40))
            self.open_with_the_only_commit()
            self.pull()["headCommit"]["nodes"][0]["commit"]["checkSuites"] = {"totalCount": 1, "nodes": [{"createdAt": PUSHED}]}
            result = self.run_cli(cutoff=None)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(f"cutoff {PUSHED};", result.stdout)
            self.assertIn("cutoff source: the first check suite on the head commit", result.stdout)

    def test_a_fast_forward_push_cuts_at_the_first_check_suite_on_the_head(self) -> None:
        self.date_the_timeline()
        self.check_the_head("2026-03-09T07:09:00Z", PUSHED, "2026-03-09T07:06:00Z")
        result = self.run_cli(cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cutoff {PUSHED};", result.stdout)
        self.assertIn("cutoff source: the first check suite on the head commit", result.stdout)

    def test_a_head_pushed_before_the_opening_cuts_at_the_opening(self) -> None:
        self.date_the_timeline()
        self.check_the_head("2026-03-07T07:29:00Z")
        result = self.run_cli(cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cutoff {OPENED};", result.stdout)
        self.assertIn("the pull request was opened after its head was pushed "
                      "(the first check suite on the head commit, 2026-03-07T07:29:00Z)", result.stdout)
        self.assertIn("(the pull request's opening)", self.out.read_text(encoding="utf-8"))

    def test_a_push_the_forge_does_not_date_is_refused_not_guessed(self) -> None:
        self.date_the_timeline(self.force_push(oid="0" * 40))
        for suites in ([], None):
            with self.subTest(suites=suites):
                if suites is not None:
                    self.check_the_head(*suites)
                result = self.run_cli(cutoff=None)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("cannot establish the last push", result.stderr)
                self.assertIn("--pushed-at", result.stderr)
                self.assertFalse(self.out.exists())

    def test_check_suites_on_another_commit_or_a_first_page_of_them_do_not_date_the_head(self) -> None:
        self.date_the_timeline()
        for change in ({"oid": "0" * 40}, {"checkSuites": {"totalCount": 101, "nodes": [{"createdAt": PUSHED}]}}):
            with self.subTest(change=change):
                self.check_the_head(PUSHED)
                self.pull()["headCommit"]["nodes"][0]["commit"].update(change)
                result = self.run_cli(cutoff=None)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("cannot establish the last push", result.stderr)

    def test_a_missing_or_truncated_timeline_exits_two(self) -> None:
        for timeline in (None, {"pageInfo": {"hasNextPage": True}, "nodes": [self.force_push()]}):
            with self.subTest(timeline=timeline):
                if timeline:
                    self.pull()["timelineItems"] = timeline
                result = self.run_cli(cutoff=None)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("timeline is missing or truncated", result.stderr)

    def test_a_given_push_instant_is_the_cutoff_and_carries_its_source(self) -> None:
        self.date_the_timeline()
        result = self.run_cli("--pushed-at", PUSHED, "--pushed-at-source", "push event in the public events archive",
                              cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cutoff {PUSHED};", result.stdout)
        self.assertIn("cutoff source: push event in the public events archive", result.stdout)
        self.assertIn(f"frozen cutoff `{PUSHED}` (the last push)", self.out.read_text(encoding="utf-8"))

    def test_a_given_push_instant_must_be_sourced_and_possible(self) -> None:
        self.date_the_timeline()
        source = ["--pushed-at-source", "push event"]
        for options, message, cutoff in [
            (["--pushed-at", PUSHED], "go together", None),
            (source, "go together", None),
            (["--pushed-at", PUSHED, *source], "not both", CUTOFF),
            (["--pushed-at", "2026-03-09T06:59:59Z", *source], "before the head was committed", None),
            (["--pushed-at", "2026-03-10T12:00:01Z", *source], "after the merge", None),
            (["--pushed-at", "yesterday", *source], "not an ISO-8601 instant", None),
        ]:
            with self.subTest(options=options):
                result = self.run_cli(*options, cutoff=cutoff)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn(message, result.stderr)
                self.assertFalse(self.out.exists())

    def test_an_unmerged_pull_request_cuts_at_its_last_push(self) -> None:
        pull = self.pull()
        pull["merged"], pull["mergedAt"], pull["state"] = False, None, "OPEN"
        self.date_the_timeline(self.force_push())
        packet = self.build_ok(cutoff=None)
        self.assertIn(f"frozen cutoff `{PUSHED}` (the last push)", packet)

    # --- text as it read at the cutoff ------------------------------------

    def test_text_edited_after_the_cutoff_reads_as_it_did_at_the_cutoff(self) -> None:
        pull = self.pull()
        comment = pull["comments"]["nodes"][0]
        for node, identifier, text in [(pull, "PR_1", "Closes #3900.\n\nNow with the reviewer's fix applied."),
                                       (comment, "IC_1", "Coverage for the merged head: 91%.")]:
            node["id"], node["lastEditedAt"], node["body"] = identifier, "2026-03-10T11:00:00Z", text
        self.save_edits(
            PR_1=[("2026-03-10T11:00:00Z", pull["body"]), ("2026-03-08T08:00:00Z", "Closes #3900.\n\nSecond draft."),
                  (OPENED, "Closes #3900.\n\nFirst draft.")],
            IC_1=[("2026-03-10T11:00:00Z", comment["body"]), ("2026-03-07T08:00:00Z", "Coverage for the first head: 88%.")])
        self.date_the_timeline(self.force_push())
        result = self.run_cli(cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("text restored to the cutoff: ['pull request body', 'conversation #1']", result.stdout)
        packet = self.out.read_text(encoding="utf-8")
        for standing in ["Second draft.", "Coverage for the first head: 88%."]:
            self.assertIn(standing, packet)
        for later in ["Now with the reviewer's fix applied.", "First draft.", "Coverage for the merged head: 91%."]:
            self.assertNotIn(later, packet)

    def test_an_edit_history_that_does_not_establish_the_text_refuses_the_packet(self) -> None:
        comment = self.pull()["comments"]["nodes"][0]
        comment["id"], comment["lastEditedAt"] = "IC_1", "2026-03-10T11:00:00Z"
        self.date_the_timeline(self.force_push())
        early, late = ("2026-03-07T08:00:00Z", "Earlier text."), ("2026-03-10T11:00:00Z", comment["body"])
        histories = {
            "no revision from the cutoff or earlier": {"totalCount": 1, "nodes": [late]},
            "truncated": {"totalCount": 3, "nodes": [late, early]},
            "two revisions at the same instant": {"totalCount": 3, "nodes": [late, early, early]},
            "deleted revision": {"totalCount": 2, "nodes": [late, (early[0], None)]},
            "absent": None,
        }
        for label, history in histories.items():
            with self.subTest(history=label):
                nodes = [] if history is None else [{"id": "IC_1", "userContentEdits": {
                    "totalCount": history["totalCount"],
                    "nodes": [{"editedAt": when, "deletedAt": None if text else "2026-03-09T00:00:00Z", "diff": text}
                              for when, text in history["nodes"]]}}]
                self.save_replay(edits={"data": {"nodes": nodes}})
                self.assert_unavailable(self.run_cli(save=False, cutoff=None), "conversation #1", "lastEditedAt")

    def test_a_title_renamed_after_the_cutoff_reads_as_it_did_at_the_cutoff(self) -> None:
        rename = {"__typename": "RenamedTitleEvent", "createdAt": "2026-03-10T11:59:00Z",
                  "previousTitle": "Bound the retry loop"}
        earlier = {"__typename": "RenamedTitleEvent", "createdAt": "2026-03-08T08:00:00Z", "previousTitle": "WIP retry"}
        later = {"__typename": "RenamedTitleEvent", "createdAt": "2026-03-10T12:30:00Z", "previousTitle": "Bound it"}
        self.date_the_timeline(later, self.force_push(), rename, earlier)
        result = self.run_cli(cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("text restored to the cutoff: ['title']", result.stdout)
        self.assertIn('— "Bound the retry loop" |', self.out.read_text(encoding="utf-8"))
        self.assertIn('— "Bound it" |', self.build_ok())

    def test_the_record_states_the_source_and_every_omission_and_restoration(self) -> None:
        comment = self.pull()["comments"]["nodes"][0]
        comment["id"], comment["lastEditedAt"] = "IC_1", "2026-03-10T11:00:00Z"
        self.save_edits(IC_1=[("2026-03-10T11:00:00Z", comment["body"]), ("2026-03-07T08:00:00Z", "Earlier text.")])
        self.date_the_timeline(self.force_push())
        record_path = self.directory / "record.json"
        result = self.run_cli("--record", str(record_path), cutoff=None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        self.assertEqual({key: record[key] for key in ["cutoff", "cutoff_is", "cutoff_source", "omitted", "title_as_of_cutoff"]}, {
            "cutoff": PUSHED, "cutoff_is": "the last push",
            "cutoff_source": "the force-push event that made this commit the head",
            "omitted": {"reviews": 2, "thread_comments": 2, "conversation": 1, "issue_comments": 1},
            "title_as_of_cutoff": None})
        self.assertIn({"kind": "reviews", "published": "2026-03-09T15:30:00Z", "author": "alice"}, record["omitted_records"])
        self.assertIn({"kind": "thread_comments", "published": "2026-03-11T09:00:00Z", "author": "dana",
                       "review_submitted": "2026-03-12T08:00:00Z"}, record["omitted_records"])
        self.assertEqual(len(record["omitted_records"]), 6)
        self.assertEqual(record["text_as_of_cutoff"], [{"record": "conversation #1", "author": "bob",
                                                        "last_edited": "2026-03-10T11:00:00Z",
                                                        "text_as_of": "2026-03-07T08:00:00Z"}])

    # --- the cutoff -----------------------------------------------------

    def test_a_cutoff_at_the_merge_instant_is_named_and_omits_what_followed(self) -> None:
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cutoff {CUTOFF}; omitted after cutoff: "
                      "{'reviews': 1, 'thread_comments': 2, 'conversation': 1, 'issue_comments': 1}",
                      result.stdout)
        self.assertIn("2 reviews, 2 thread comments, 1 conversation comments, 1 issues", result.stdout)

    def test_material_after_the_cutoff_is_omitted(self) -> None:
        packet = self.build_ok()
        for kept in ["The retry loop needs a bound, not a longer sleep.",
                     "Bound this loop.",
                     "Bounded in the follow-up commit.",
                     "Opening for review; the ceiling is configurable.",
                     "Reproduced on 1.2.0 with a proxy that closes mid-handshake."]:
            self.assertIn(kept, packet)
        for omitted in ["downstream breakage report",
                        "Still spinning in production after the merge.",
                        "Filed #4001 for the fallout.",
                        "Reverted in #4002.",
                        "Reopening: the shipped fix still hot-loops"]:
            self.assertNotIn(omitted, packet)

    def test_a_thread_emptied_by_the_cutoff_is_dropped(self) -> None:
        packet = self.build_ok()
        self.assertIn("### Review threads (1), comments verbatim, in order", packet)
        self.assertNotIn("src/lib.rs:7", packet)

    def test_the_packet_states_the_cutoff_and_not_the_omitted_count(self) -> None:
        packet = self.build_ok()
        self.assertIn(f"## 6. Prior review state through the frozen cutoff `{CUTOFF}` (the merge instant)", packet)
        self.assertIn(f"### Issue comments through the frozen cutoff `{CUTOFF}`", packet)
        self.assertNotIn("omitted", packet.lower())

    def test_an_explicit_earlier_cutoff_drops_more(self) -> None:
        earlier = "2026-03-08T09:30:00Z"
        result = self.run_cli("--cutoff", earlier)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cutoff {earlier}; omitted after cutoff: "
                      "{'reviews': 2, 'thread_comments': 3, 'conversation': 1, 'issue_comments': 1}",
                      result.stdout)
        packet = self.out.read_text(encoding="utf-8")
        self.assertIn(f"## 6. Prior review state through the frozen cutoff `{earlier}`, reproduced verbatim", packet)
        self.assertNotIn("Bounded in the follow-up commit.", packet)

    def test_the_pinned_merge_instant_survives_an_earlier_cutoff(self) -> None:
        packet = self.build_ok("--cutoff", "2026-03-08T09:30:00Z")
        self.assertIn("| `merged` | **`true`** (merged 2026-03-10T12:00:00Z) |", packet)

    def test_a_comment_whose_review_was_submitted_after_the_cutoff_is_omitted(self) -> None:
        thread = self.fixture["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"][0]
        thread["comments"]["nodes"][1]["pullRequestReview"] = {"submittedAt": "2026-03-11T09:00:00Z"}
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("'thread_comments': 3", result.stdout)
        self.assertNotIn("Bounded in the follow-up commit.", self.out.read_text(encoding="utf-8"))

    def test_a_standalone_thread_comment_within_the_cutoff_is_kept(self) -> None:
        packet = self.build_ok()
        self.assertIn("Bounded in the follow-up commit.", packet)

    def required_graphql_sources(self):
        pull = self.fixture["data"]["repository"]["pullRequest"]
        issue = pull["closingIssuesReferences"]["nodes"][0]
        return [
            ("pull request body", pull, "createdAt"),
            ("issue #3900 body", issue, "createdAt"),
            ("reviews", pull["reviews"]["nodes"][0], "submittedAt"),
            ("thread_comments", pull["reviewThreads"]["nodes"][0]["comments"]["nodes"][0], "createdAt"),
            ("conversation", pull["comments"]["nodes"][0], "createdAt"),
            ("issue_comments", issue["comments"]["nodes"][0], "createdAt"),
        ]

    def assert_unavailable(self, result, *messages):
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("required input unavailable", result.stdout)
        for message in messages:
            self.assertIn(message, result.stdout)
        self.assertFalse(self.out.exists())

    def test_required_graphql_timestamps_must_be_aware_and_valid(self) -> None:
        for label, node, key in self.required_graphql_sources():
            original = node[key]
            for stamp in [None, "invalid", "2026-03-08", "2026-03-08T09:00:00",
                          "2026-03-08T09:00:00+1260", "2026-03-10T12:00:00.0000001Z", 123]:
                with self.subTest(source=label, stamp=stamp):
                    node[key] = stamp
                    self.assert_unavailable(self.run_cli(), label, key)
            del node[key]
            self.assert_unavailable(self.run_cli(), label, key)
            node[key] = original

    def test_required_review_submission_cannot_be_absent(self) -> None:
        node = self.required_graphql_sources()[3][1]
        for stamp in [None, "invalid", "2026-03-08T09:00:00"]:
            node["pullRequestReview"] = {"submittedAt": stamp}
            self.assert_unavailable(self.run_cli(), "submittedAt")
        del node["pullRequestReview"]
        self.assert_unavailable(self.run_cli(), "pullRequestReview")

    def test_edited_or_unknown_graphql_text_refuses_the_packet(self) -> None:
        for label, node, _ in self.required_graphql_sources():
            for stamp in ["2026-03-11T09:00:00Z", "invalid", "2026-03-08T09:00:00"]:
                with self.subTest(source=label, stamp=stamp):
                    node["lastEditedAt"] = stamp
                    self.assert_unavailable(self.run_cli(), label, "lastEditedAt")
            del node["lastEditedAt"]
            self.assert_unavailable(self.run_cli(), label, "lastEditedAt")
            node["lastEditedAt"] = None

    def test_material_edited_before_or_at_the_cutoff_is_kept(self) -> None:
        for _, node, _ in self.required_graphql_sources():
            node["lastEditedAt"] = CUTOFF
        packet = self.build_ok()
        self.assertIn("The retry loop needs a bound, not a longer sleep.", packet)

    def test_graphql_sources_keep_equivalent_cutoff_instants(self) -> None:
        for stamp in [CUTOFF, "2026-03-10T17:30:00+05:30", "2026-03-10T17:30:00+0530"]:
            with self.subTest(stamp=stamp):
                for _, node, key in self.required_graphql_sources():
                    node[key] = stamp
                self.build_ok("--cutoff", stamp)
                self.out.unlink()

    def test_required_bodies_created_after_cutoff_are_unavailable(self) -> None:
        for label, node, key in self.required_graphql_sources()[:2]:
            original = node[key]
            node[key] = "2026-03-11T09:00:00Z"
            self.assert_unavailable(self.run_cli(), label)
            node[key] = original

    def test_late_history_is_omitted_even_if_edited_later(self) -> None:
        pull = self.fixture["data"]["repository"]["pullRequest"]
        pull["comments"]["nodes"][1]["lastEditedAt"] = "2026-03-20T09:00:00Z"
        self.assertNotIn("Reverted in #4002.", self.build_ok())

    def test_future_dates_in_prose_are_preserved(self) -> None:
        for stamp in ["2026-03-20T09:00:00Z", "2026-03-20T09:00:00+0530"]:
            for _, node, _ in self.required_graphql_sources():
                node["body"] = "Planned deadline: " + stamp
            packet = self.build_ok()
            self.assertEqual(packet.count("Planned deadline: " + stamp), 6)
            self.out.unlink()

    def test_rest_source_provenance_on_both_routes(self) -> None:
        self.drop_the_closing_reference()
        for option, name, source, comments in [
            ("--ref-pr", "ref_pr", REF_PR, REF_PR_COMMENTS),
            ("--spec-issue", "spec_issue", SPEC_ISSUE, SPEC_ISSUE_COMMENTS),
        ]:
            value = "3899" if option == "--ref-pr" else "other/spec#12"
            for target in ["body", "comment"]:
                for key in ["created_at", "updated_at"]:
                    for stamp in ["missing", None, "invalid", "2026-03-08T09:00:00", "2026-03-11T09:00:00Z"]:
                        # A newly published comment is omitted; an unavailable older body is not.
                        if target == "comment" and key == "created_at" and stamp == "2026-03-11T09:00:00Z":
                            continue
                        with self.subTest(route=option, target=target, key=key, stamp=stamp):
                            body, cs = copy.deepcopy(source), copy.deepcopy(comments)
                            node = body if target == "body" else cs[0]
                            if stamp == "missing":
                                del node[key]
                            else:
                                node[key] = stamp
                            self.save_replay(**{name: body, name + "_comments": cs})
                            self.assert_unavailable(self.run_cli(option, value, save=False), option, key)

    def test_rest_comments_at_cutoff_and_before_are_kept(self) -> None:
        self.drop_the_closing_reference()
        for option, name, source, comments in [
            ("--ref-pr", "ref_pr", REF_PR, REF_PR_COMMENTS),
            ("--spec-issue", "spec_issue", SPEC_ISSUE, SPEC_ISSUE_COMMENTS),
        ]:
            cs = copy.deepcopy(comments)
            cs[0]["created_at"] = "2026-03-10T17:30:00+0530"
            cs[0]["updated_at"] = CUTOFF
            self.save_replay(**{name: source, name + "_comments": cs})
            value = "3899" if option == "--ref-pr" else "other/spec#12"
            packet = self.build_ok(option, value, save=False)
            self.assertIn(cs[0]["body"], packet)
            self.assertNotIn(cs[1]["body"], packet)
            self.out.unlink()

    def test_rest_truncation_on_both_routes_refuses_the_packet(self) -> None:
        self.drop_the_closing_reference()
        for option, name, source, comments in [
            ("--ref-pr", "ref_pr", REF_PR, REF_PR_COMMENTS),
            ("--spec-issue", "spec_issue", SPEC_ISSUE, SPEC_ISSUE_COMMENTS),
        ]:
            value = "3899" if option == "--ref-pr" else "other/spec#12"
            self.save_replay(**{name: dict(source, comments=101), name + "_comments": [comments[0]] * 100})
            self.assert_unavailable(self.run_cli(option, value, save=False), option, "100 of 101")
            body = dict(source)
            del body["comments"]
            self.save_replay(**{name: body, name + "_comments": comments})
            self.assert_unavailable(self.run_cli(option, value, save=False), option, "count")

    def test_unused_rest_options_do_not_require_history(self) -> None:
        # GraphQL closing references already supply section 4; neither REST source is required.
        self.build_ok("--ref-pr", "3899", "--spec-issue", "other/spec#12")

    def test_a_fractional_cutoff_is_stated_without_losing_precision(self) -> None:
        packet = self.build_ok("--cutoff", "2026-03-10T12:00:00.123456Z")
        self.assertIn("2026-03-10T12:00:00.123456Z", packet)

    # --- the mirror -----------------------------------------------------

    def test_the_manifest_and_commits_come_from_the_mirror(self) -> None:
        packet = self.build_ok()
        self.assertIn("A  src/lib.rs", packet)
        self.assertIn("M  src/retry.rs", packet)
        self.assertIn("| Diff | 2 files, +2 / −1, 1 commits |", packet)
        self.assertIn(f"| 1 | `{self.head[:9]}` | 2026-03-09 | Bob | Bound the reconnect retry loop |", packet)

    def test_a_rename_names_real_paths_in_the_manifest_and_the_guidance_scope(self) -> None:
        self.rename_into_guide()
        packet = self.build_ok()
        self.assertIn("D  docs/notes.md", packet)
        self.assertIn("A  guide/notes.md", packet)
        self.assertNotIn("=>", packet)
        self.assertIn("| `guide/AGENTS.md` | **yes** |", packet)

    def test_a_path_git_would_quote_reaches_the_manifest_and_the_guidance_scope_raw(self) -> None:
        self.touch_the_quoted_path()
        packet = self.build_ok()
        self.assertIn(f"M  {QUOTED_DIRECTORY}/a.md", packet)
        self.assertIn(f"| `{QUOTED_DIRECTORY}/AGENTS.md` | **yes** |", packet)
        self.assertNotIn('\\"', packet)

    def test_a_tab_in_a_path_reaches_manifest_and_scoped_guidance(self) -> None:
        path = 'docs/tab\tdir/a\tb.md'
        self.write(self.mirror / path, "Tab-bearing filename.\n")
        self.write(self.mirror / 'docs/tab\tdir/AGENTS.md', "Scoped guidance.\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "Add guidance")
        self.merge_base = self.git("rev-parse", "HEAD")
        self.write(self.mirror / path, "Changed tab-bearing filename.\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "Change tab-bearing path")
        self.head = self.git("rev-parse", "HEAD")
        self.fixture["data"]["repository"]["pullRequest"]["headRefOid"] = self.head
        packet = self.build_ok()
        self.assertIn("M  " + path, packet)
        self.assertIn('| `docs/tab\tdir/AGENTS.md` | **yes** |', packet)

    def test_guidance_at_the_merge_base_covers_root_and_scoped_files(self) -> None:
        packet = self.build_ok()
        self.assertIn("| `AGENTS.md` | **yes** |", packet)
        self.assertIn("| `CLAUDE.md` | no | — |", packet)
        self.assertIn("| `src/AGENTS.md` | **yes** |", packet)
        self.assertNotIn("| `src/CLAUDE.md` |", packet)

    # --- the optional sections -----------------------------------------

    def test_an_originating_pull_request_reference_renders_as_the_spec(self) -> None:
        self.drop_the_closing_reference()
        self.save_replay(ref_pr=REF_PR, ref_pr_comments=REF_PR_COMMENTS)
        packet = self.build_ok("--ref-pr", "3899", save=False)
        self.assertIn("## 4. Originating reference `#3899` (a pull request, closed unmerged), verbatim", packet)
        self.assertIn("Closing in favour of the bounded loop.", packet)
        self.assertNotIn("Post-merge note that must not reach the packet.", packet)
        self.assertIn("closed 2026-03-10T12:00:05Z when the reviewed pull request merged.", packet)

    def test_a_cross_repository_spec_issue_renders_as_the_spec(self) -> None:
        self.drop_the_closing_reference()
        self.save_replay(spec_issue=SPEC_ISSUE, spec_issue_comments=SPEC_ISSUE_COMMENTS)
        packet = self.build_ok("--spec-issue", "other/spec#12", save=False)
        self.assertIn("## 4. User-supplied spec: `other/spec#12`, verbatim", packet)
        self.assertIn("A ceiling of five attempts is enough.", packet)
        self.assertNotIn("Post-merge note that must not reach the packet.", packet)

    def test_a_pull_request_with_no_reference_says_so(self) -> None:
        self.drop_the_closing_reference()
        packet = self.build_ok()
        self.assertIn("## 4. Originating issue\n\nNone.", packet)

    def test_the_extra_section_lands_before_the_run_conditions(self) -> None:
        extra = self.directory / "extra.md"
        extra.write_text("## 7b. Upstream material\n\nSee `upstream/`.\n", encoding="utf-8")
        packet = self.build_ok("--extra-section", str(extra))
        self.assertLess(packet.index("## 7b. Upstream material"), packet.index("## 8. Run conditions"))

    def test_publish_to_fork_switches_the_run_conditions(self) -> None:
        packet = self.build_ok("--publish-to-fork")
        self.assertIn("**Publication is ENABLED**", packet)
        self.assertNotIn("**Publication is disabled.**", packet)
        self.assertIn("publication is ENABLED**", packet)

    def test_the_program_strings_default_to_the_124_experiment(self) -> None:
        packet = self.build_ok("--publish-to-fork")
        self.assertIn("(target (a), issue #124 effort experiment)", packet)
        self.assertIn("the original author is `scop`", packet)
        self.assertIn("no access to `spf13/cobra`", packet)
        self.assertIn('**pass `model: "sonnet"` explicitly on every call**', packet)

    def test_another_program_states_its_own_label_and_identities(self) -> None:
        packet = self.build_ok("--publish-to-fork",
                               "--experiment-label", "issue #137 recall grid",
                               "--subagent-model", "opus",
                               "--upstream-repo", "example/upstream",
                               "--original-author", "frank")
        self.assertIn("(target (a), issue #137 recall grid)", packet)
        self.assertIn("the original author is `frank`", packet)
        self.assertIn("no access to `example/upstream`", packet)
        self.assertIn('**pass `model: "opus"` explicitly on every call**', packet)
        for stale in ["issue #124 effort experiment", "`scop`", "spf13/cobra", 'model: "sonnet"']:
            self.assertNotIn(stale, packet)

    def test_the_execution_note_reaches_the_run_conditions(self) -> None:
        packet = self.build_ok("--execution-note", "Focused tests only, five minutes each.")
        self.assertIn("Focused tests only, five minutes each.", packet)

    # --- the factual packet ----------------------------------------------

    def test_the_factual_packet_is_the_derived_rendering(self) -> None:
        sys.path.insert(0, str(SCRIPT.parent))
        import derive_packet
        for drop in (False, True):
            with self.subTest(closing_reference=not drop):
                if drop:
                    self.drop_the_closing_reference()
                rendered = self.build_ok()
                factual = self.build_ok("--factual")
                self.assertEqual(factual, derive_packet.derive(rendered))
                for policy in ["## 8. Run conditions", "Posting identity", "review-head", "issues=",
                               "summary.repository_url", "Mandatory note", "issue #124 effort experiment"]:
                    self.assertNotIn(policy, factual)

    def test_a_factual_rendering_the_derivation_refuses_writes_nothing(self) -> None:
        self.drop_the_closing_reference()
        self.save_replay(spec_issue=SPEC_ISSUE, spec_issue_comments=SPEC_ISSUE_COMMENTS)
        result = self.run_cli("--spec-issue", "other/spec#12", "--factual", save=False)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("expected exactly one", result.stdout)
        self.assertFalse(self.out.exists())

    # --- refusals -------------------------------------------------------

    def test_a_head_mismatch_exits_two(self) -> None:
        self.fixture["data"]["repository"]["pullRequest"]["headRefOid"] = "0" * 40
        result = self.run_cli()
        self.assertEqual(result.returncode, 2)
        self.assertIn("head mismatch", result.stderr)
        self.assertFalse(self.out.exists())

    def test_a_missing_replay_response_exits_two(self) -> None:
        result = self.run_cli(save=False)
        self.assertEqual(result.returncode, 2)
        self.assertIn("cannot read", result.stderr)

    def test_an_unmerged_pull_request_builds_with_a_cutoff(self) -> None:
        pull = self.fixture["data"]["repository"]["pullRequest"]
        pull["merged"], pull["mergedAt"], pull["state"] = False, None, "OPEN"
        packet = self.build_ok("--cutoff", CUTOFF)
        self.assertIn("| `merged` | **`false`** (not merged) |", packet)
        self.assertNotIn("merged None", packet)
        self.assertIn("the target is not merged and this run does not publish", packet)
        self.assertIn("The target is not merged; this review is frozen at the cutoff.", packet)
        self.assertNotIn("The target is merged; this is a retrospective review.", packet)
        self.assertIn(f"## 6. Prior review state through the frozen cutoff `{CUTOFF}`, reproduced verbatim", packet)

    def test_a_truncated_connection_exits_one(self) -> None:
        pull = self.fixture["data"]["repository"]["pullRequest"]
        pull["reviews"]["totalCount"] = 101
        pull["reviewThreads"]["nodes"][0]["comments"]["totalCount"] = 50
        result = self.run_cli()
        self.assert_unavailable(result, "reviews (3 of 101)", "comments on the src/retry.rs:42 thread (3 of 50)")
        self.assertFalse(self.out.exists())

    def test_every_graphql_collection_requires_complete_counts(self) -> None:
        pull = self.fixture["data"]["repository"]["pullRequest"]
        connections = [pull[key] for key in ["reviews", "reviewThreads", "comments", "closingIssuesReferences"]]
        connections += [pull["reviewThreads"]["nodes"][0]["comments"],
                        pull["closingIssuesReferences"]["nodes"][0]["comments"]]
        for connection in connections:
            total = connection.pop("totalCount")
            self.assert_unavailable(self.run_cli(), "count")
            connection["totalCount"] = total + 1
            self.assert_unavailable(self.run_cli(), "incomplete collection")
            connection["totalCount"] = total

    def test_complete_connections_build(self) -> None:
        packet = self.build_ok()
        self.assertIn("### Review submissions (2)", packet)

    def test_a_malformed_cutoff_exits_two(self) -> None:
        result = self.run_cli("--cutoff", "last tuesday")
        self.assertEqual(result.returncode, 2)
        self.assertIn("not an ISO-8601 instant", result.stderr)

    def test_an_unreadable_extra_section_exits_two(self) -> None:
        result = self.run_cli("--extra-section", str(self.directory / "absent.md"))
        self.assertEqual(result.returncode, 2)
        self.assertIn("cannot read", result.stderr)

    def test_a_staging_mirror_without_the_pinned_shas_exits_two(self) -> None:
        empty = self.directory / "empty"
        empty.mkdir()
        subprocess.run(["git", "init", "-q", "-b", "main", str(empty)], check=True, capture_output=True)
        self.save_replay()
        result = self.run_cli("--staging", str(empty), save=False)
        self.assertEqual(result.returncode, 2)
        self.assertIn("command failed: git", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
