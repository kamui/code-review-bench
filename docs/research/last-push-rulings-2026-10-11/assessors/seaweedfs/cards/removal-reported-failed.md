# Card: a file left unlisted when a removal that ran is reported as failed

## Background

Pull request: seaweedfs/seaweedfs #10735. Before it, a directory listing skipped a name whose value was gone and left the name in the list. With it, the listing removes such a name, checks whether the value is back, and if so puts the name back. Each fact is marked *run* (executed, probe saved), *read* or *reported*.

The Redis filer store (`redis2`, and the `redis_cluster2` and `redis2_sentinel` stores that share its code) keeps two things for every file:

- a *value*: the file's metadata, stored under the file's path;
- the file's *name in its directory's list*, a Redis sorted set.

A directory listing reads the list, then fetches each value. A name whose value is gone, for example because its time-to-live ran out, is an *orphan*.

What goes wrong: the cleanup returns at once when its removal reports an error, on the assumption that nothing was removed. The client can report an error for a removal that Redis did run.

## What the affected person sees

No error anywhere, both requests report success, and nothing is logged. The file is missing from every later listing of its directory. It can be read by its full path. An overwrite does not bring it back, and a recursive delete leaves its value. It comes back only after a delete and re-create by exact path.

Before the change, in the same sequence, the file is listed (*run*).

## Impact facts

- **Who is affected.** The person whose file it is, on a filer that uses one of the three `redis2`-family stores.
- **How.** A file that was stored successfully is absent from its directory's listing, with success reported by both requests. Its data and value are intact.
- **How stuck.** Fully, for that one file, until someone who knows the exact path deletes and re-creates it (*run*).
- **Whose behaviour changed.** SeaweedFS's. The base issues no removal on this branch (*run*).
- **What it takes at this site by itself.** The path is created again in the one-round-trip gap, and then either a single lost reply with retries switched off, or Redis running commands without answering for long enough that the removal gives up. With default settings a single lost reply is repaired by the client's own retry (*run*).
- **How common.** Not known. No public configuration that switches retries off was found. How often a re-create lands in the gap, and how often Redis goes silent just then, nobody measured.
- **What reveals it.** Nothing, except comparing a lookup by path with the directory listing.
- **Limits of the evidence.** The command order was forced, not raced. One standalone Redis was used. A TCP relay stood in for a failing server. The silent-Redis case used a 150 ms read timeout, not the default 3 s.
