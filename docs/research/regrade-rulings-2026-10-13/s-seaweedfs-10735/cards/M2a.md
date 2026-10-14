# Card: the expiry branch removes the name of a file that was just created again

Pull request: seaweedfs/seaweedfs #10735. SeaweedFS is a distributed file store; its *filer* keeps file metadata in a pluggable store, here Redis (`redis2`). The store keeps a *record* for each file and the file's *name* in its directory's list. A listing reads names, then fetches each record. "Base" is the commit before the change and "head" the reviewed commit.

The listing has a branch for a file whose creation time plus time-to-live has passed while Redis still holds its record: it deletes the record, then removes the name. That branch is older than the pull request and unchanged by it. The pull request edits the neighbouring branch, for a name whose record is already gone, and describes its first commit as mirroring this one.

## The runs

The real store and filer code against a real Redis 8.2.1. The two requests are not raced: the order of commands was forced, and each run prints the order Redis saw. No error is injected. Probes and outputs: `probes/M2/`, `probes/M2a/`. All *run*.

The order, the same at both commits:

```text
1. lister:   reads the names of the directory          -> [f]
2. lister:   reads the record of f                     -> an expired file
3. lister:   DEL  the record of f                      -> 1
4. creator:  SET  a new record of f                    -> OK
5. creator:  ZADD NX the name f                        -> 0 (already there)
6. creator:  the create returns success
7. lister:   ZREM the name f                           -> 1
```

| What is run | Base | Head |
| --- | --- | --- |
| a complete create of `f` between the listing's delete and its removal of the name (store level) | the record exists; the name is gone; later listings do not show `f` | the same |
| the same through the filer on both sides, the create being what an upload runs | the upload returns success; `f` is found by its full path; later listings do not show it | the same |
| the listing alone | record and name both removed | the same |
| nothing forced: a file stored with a time-to-live, stored again before it ran out, then listed after the first time-to-live | Redis still holds the record; the listing deletes it and removes the name | the same |

Afterwards, at both commits: an overwrite of the file succeeds and it is still not listed; a recursive delete of the directory leaves its record; a new create at the store lists it again (the delete that would come before it was not run).

In the neighbouring branch, the one the pull request edits, the same create at the same moment keeps its name at the base and at the head (*run*).

## What the affected person sees

A program stores a file whose earlier copy is past its time-to-live, at the moment a listing of that directory is between two commands:

- the store request returns success;
- the file reads by its full path;
- it is missing from every later listing;
- storing it again over itself does not bring it back, and a recursive delete of the directory leaves it behind;
- no error and no log line.

Before the change the same sequence gave the same result.

## Impact facts

- **Who is affected:** a person or program that stores a file again at a path whose earlier copy had a time-to-live, on a filer that uses the Redis store.
- **How:** a file that was stored successfully is absent from its directory's listing. Its record is intact.
- **How stuck:** for that one file, until someone who knows its exact path deletes it and creates it again. Nothing tells them to.
- **How common:** not measured. It takes a file past its time-to-live whose record Redis still holds (a file that was stored again before the time-to-live ran out), a listing of its directory, and a complete create of the same path between two consecutive commands of that listing, one Redis round trip. No report of it was found before the cut-off. Three public repositories turn this store on; one of them sets a time-to-live on bucket folders.
- **Neighbours:** the previous commit behaves the same way in this sequence. The documentation says "The Filer Store persists all file metadata and directory information." A maintainer wrote in 2023, of a directory on a Redis store that could not be listed: "In redis, the operations to "add a file" and "add the file to the directory" are not in the same transaction. So this may happen." The pull request's head commit describes the same end state, a file with a record and no name, as the fault it fixes in the neighbouring branch.
- **Limits of the evidence:** the order was forced, not raced; one standalone Redis; the cluster and sentinel variants and the HTTP and S3 gateways were not run.
