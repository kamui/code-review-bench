# Card: a file left unlisted when a cleanup command fails after the removal, by either of two routes

## Background

Pull request: seaweedfs/seaweedfs #10735. Before it, a directory listing skipped a name whose value was gone and left the name in the list. With it, the listing removes such a name, checks whether the value is back, and if so puts the name back. Each fact is marked *run* (executed, probe saved), *read* or *reported*.

The Redis filer store (`redis2`, and the `redis_cluster2` and `redis2_sentinel` stores that share its code) keeps two things for every file:

- a *value*: the file's metadata, stored under the file's path;
- the file's *name in its directory's list*, a Redis sorted set.

A directory listing reads the list, then fetches each value. A name whose value is gone, for example because its time-to-live ran out, is an *orphan*.

What goes wrong: once the removal has been sent, an error can leave a live file without its name. Either the command that puts the name back fails and its result is thrown away, or the removal itself ran and is reported as failed, and the cleanup returns at once.

## What the affected person sees

The affected person is a person or program using a SeaweedFS filer on the `redis2`, `redis_cluster2` or `redis2_sentinel` store, with files whose value can vanish while the name stays listed (a time-to-live, eviction, a direct key delete), where a path is written again after that.

When it happens, at the head (*run*):

- No error anywhere. The listing returns success. The request that created the file returned success. Nothing is logged about it.
- The file is missing from every listing of its directory from then on.
- The file can still be read by its full path.
- Writing the file again does not bring it back.
- Deleting the directory recursively does not delete the file's value.
- It comes back only when someone deletes the file by its exact path and creates it again. Nothing tells them to.

Before the change, in the same sequence, the file is listed (*run*).

## Impact facts

- **Who is affected.** The person whose file it is, on a filer that uses one of the three `redis2`-family stores. Three public repositories were found that turn `redis2` on.
- **How.** A file that was stored successfully is absent from its directory's listing. Both requests involved reported success. The file's data and value are intact and readable by full path.
- **How stuck.** Fully, for that one file, until someone who knows the exact path deletes and re-creates it. No later listing, overwrite or recursive delete repairs it (*run*). No setting turns the cleanup off (*read*).
- **Whose behaviour changed.** SeaweedFS's. The base issues no removal on this branch, and keeps the name in every scenario (*run*).
- **What the first route takes (the put-back fails).** Two things at once. First, the path is created again in the gap between two consecutive commands of one listing, one Redis round trip. Second, the put-back fails: Redis unreachable from just after the removal through the client's retries of the next two commands (3.4 s in the run), or Redis refusing writes at its memory limit. A fault alone loses nothing. A re-create alone loses nothing. A fault that starts before the removal loses nothing (*run*).
- **What the second route takes.** The path is created again in the one-round-trip gap, and then either a single lost reply with retries switched off, or Redis running commands without answering for long enough that the removal gives up. With default settings a single lost reply is repaired by the client's own retry (*run*).
- **What does not cause it.** A client disconnecting during a listing: through the filer the context is detached, and the name was put back (*run*).
- **How common.** Not known. Nobody measured how often a re-create lands in the gap or how often a fault follows it. How many Redis servers behind a filer run with a memory limit is not known either.
- **What reveals it.** Nothing at the head, except comparing a lookup by path with the directory listing.
- **Limits of the evidence.** The command order was forced, not raced. One standalone Redis was used. A TCP relay stood in for a failing server.
