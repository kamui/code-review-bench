# Review blind-6650e2

### Item 1
Location: weed/filer/redis2/universal_redis_store.go:208
Claim: Replica-lag NotFound makes the lister ZREM a live index member for good
Consequence: With redis_cluster2 and useReadOnly or routeByLatency, the value key and the index key live in different slots, so GET, EXISTS and ZRANGEBYLEX can hit different, differently lagged replicas. A just-created entry whose index member has replicated but whose value has not makes FindEntry return ErrNotFound; the master ZREM removes the member, and the EXISTS recheck goes to the same lagging replica, sees 0, and does not restore it. UpdateEntry never re-adds the member, so the live entry stays out of listings and DeleteFolderChildren until the same path is inserted again. Before this change the same lag only produced a transient skip. The PR claims the compensation behaves identically on all three transports, which does not hold for replica reads.
Fix: Do not prune on a read that may have come from a replica: skip removeOrphanedDirectoryListMember when RedisCluster2Store was initialized with readOnly or routeByLatency (keep a flag on UniversalRedis2Store), or confirm absence against the master before ZREM. Assumption: only the cluster store exposes replica reads; the sentinel store uses a master-only failover client.
