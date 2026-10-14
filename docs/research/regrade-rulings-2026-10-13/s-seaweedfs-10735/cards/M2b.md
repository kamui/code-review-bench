# Card: the expiry branch deletes the record of a file that was just created again

Pull request: seaweedfs/seaweedfs #10735. SeaweedFS is a distributed file store; its *filer* keeps file metadata in a pluggable store, here Redis (`redis2`). The store keeps a *record* for each file and the file's *name* in its directory's list. A listing reads names, then fetches each record. "Base" is the commit before the change and "head" the reviewed commit.

The listing has a branch for a file whose creation time plus time-to-live has passed while Redis still holds its record: it reads the record, decides the file has expired, deletes whatever record is at that path, then removes the name. That branch is older than the pull request and unchanged by it. The pull request edits the neighbouring branch, for a name whose record is already gone.

## The runs

The real store and filer code against a real Redis 8.2.1. The two requests are not raced: the order of commands was forced, and each run prints the order Redis saw. No error is injected. Probes and outputs: `probes/M2/`, `probes/M2b/`. All *run*.

The order, the same at both commits:

```text
1. lister:   reads the names of the directory          -> [f]
2. lister:   reads the record of f                     -> an expired file
3. creator:  SET  a new record of f                    -> OK
4. creator:  ZADD NX the name f                        -> 0 (already there)
5. creator:  the create returns success
6. lister:   DEL  the record of f                      -> 1
7. lister:   ZREM the name f                           -> 1
```

| What is run | Base | Head |
| --- | --- | --- |
| a complete create of `f` between the listing's read of `f` and its delete (store level) | the create returns success; afterwards there is no record and no name; a read by path says not found | the same |
| the same through the filer on both sides, the create being what an upload runs | the upload returns success; afterwards a read by full path says not found, and no listing shows `f` | the same |
| the listing alone | record and name both removed | the same |
| nothing forced: a file stored with a time-to-live, stored again before it ran out, then listed after the first time-to-live | Redis still holds the record; the listing deletes it and removes the name | the same |

## What the affected person sees

A program stores a file whose earlier copy is past its time-to-live, at the moment a listing of that directory has read the old copy and not yet deleted it:

- the store request returns success;
- afterwards the file is not found by its path and is in no listing;
- no error and no log line;
- storing it once more works (read, not run).

Before the change the same sequence gave the same result.

## Impact facts

- **Who is affected:** a person or program that stores a file again at a path whose earlier copy had a time-to-live, on a filer that uses the Redis store.
- **How:** a store request that reported success leaves no file. The listing's delete removes the record only; what the upload wrote to a volume server is not removed by it (read, not run).
- **How stuck:** the program is not told. It learns of it only by reading the file back. Storing the file again repairs it (read, not run).
- **How common:** not measured. It takes a file past its time-to-live whose record Redis still holds (a file that was stored again before the time-to-live ran out), a listing of its directory, and a complete create of the same path between the listing's read of that file and its delete. No report of it was found before the cut-off. Three public repositories turn this store on; one of them sets a time-to-live on bucket folders.
- **Neighbours:** the previous commit behaves the same way in this sequence. The documentation says "The Filer Store persists all file metadata and directory information." By the time of the reviewed commit the pull request's text speaks of a create losing its *name* to the listing in the neighbouring branch; it does not mention a record being deleted.
- **Limits of the evidence:** the order was forced, not raced; one standalone Redis; the cluster and sentinel variants and the HTTP and S3 gateways were not run.
