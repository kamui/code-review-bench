import json, hashlib
from pathlib import Path
from collections import Counter
OUT=Path(__file__).parent
ROOT=Path('/home/jack/.t3/worktrees/code-review-bench/t3code-7fc50325')
E='docs/research/selected-pr-adjudication-2026-09-30/evidence/'
ground=json.loads(Path('/tmp/arena-selected-pr-triage-2026-09-30/grounding.json').read_text())
issues=[]
def add(id,target,summary,tokens,outcome,route,obligation,trigger,attribution,consequence,recommendation,support,opposition,limits,paths,question=None):
    row=dict(id=id,target=target,summary=summary,original_tokens=tokens.split(),proposed_outcome=outcome,route=route,exact_unresolved_question=question,recommendation=recommendation,why=recommendation,supporting_evidence=support,opposing_evidence=opposition,evidence_limits=limits,evidence_paths=[p if p.startswith('/') else E+p for p in paths],administrative_approval_needed=True,assessment=dict(concrete_obligation=obligation,reachable_supported_trigger=trigger,attribution=attribution,material_consequence=consequence))
    issues.append(row)
    return row
u='u-grpc-go-6919';v='v-django-17914';w='w-graphql-js-3457';y='y-django-16631';x='x-kubernetes-141463'
add('U-codec-v1',u,'Default protobuf codec rejects previously supported V1-only messages','R030 R051 R054 R057 R060 R063','eligible','clear',
    'Retain marshal and unmarshal support for existing generated protobuf messages.',
    'An RPC uses a registered legacy generated message without ProtoReflect through the default codec.',
    'Introduced. The import switches both assertions from V1 to V2 while preserving the public codec interface.',
    'Requests or responses fail serialization and RPCs fail, rather than merely changing an internal representation.',
    'Approve eligibility routinely. The repository fixture and base-to-head probe directly demonstrate a compatibility failure.',
    ['The base codec marshals 13 bytes and unmarshals successfully. Head returns want proto.Message for both operations.','SearchRequestV3 implements V1 without ProtoReflect.'],
    ['V2 messages still work. Restoring status detail adaptation alone cannot repair the codec.'],
    ['The saved check exercises the actual codec but does not run a network RPC. RPC consequences follow from the codec contract.'],
    [u+'/change.diff',u+'/base/encoding/proto/proto.go',u+'/head/encoding/proto/proto.go',u+'/head/reflection/grpc_testing_not_regenerate/testv3.go','probes/grpc-probe.go','probes/grpc-base.txt','probes/grpc-head.txt'])
add('U-status-v1',u,'Details returns a wrapper instead of the original legacy detail type','R007 R017 R019 R023 R059 R064','eligible','clear',
    'Preserve concrete detail types returned by successful WithDetails and Details round trips.',
    'A registered V1-only protobuf message is attached to a non-OK status, decoded, and type-asserted by the caller.',
    'Introduced. Any.UnmarshalNew replaces ptypes.UnmarshalAny and the old V1 unwrapping disappears.',
    'Existing type switches fail and unchecked type assertions panic after successful detail serialization.',
    'Approve eligibility routinely. The fixture round trip demonstrates the public compatibility regression and the archived fix identifies this exact migration.',
    ['Saved base returns SearchRequestV3. Head returns impl.messageIfaceWrapper.','WithDetails still deliberately accepts MessageV1 and adapts it to MessageV2.','Archived PR 7724 describes and repairs the exact return-type regression from PR 6919.'],
    ['Successfully encoding the detail does not establish return-type compatibility.','The later upstream fix supports technical attribution but supplies no benchmark human approval.'],
    ['No network RPC is necessary for this public status API failure. Upstream disposition is specific to status, not every V1 path.'],
    [u+'/change.diff',u+'/base/internal/status/status.go',u+'/head/internal/status/status.go','probes/grpc-probe.go','probes/grpc-base.txt','probes/grpc-head.txt','upstream/sol/sources/grpc-grpc-go-7724/pr.json'])
add('U-binary-reply-v1',u,'Unary server binary logging loses legacy response payloads','R004 R010 R021 R034 R050 R066','eligible','clear',
    'Record the serialized protobuf reply when binary logging a successful supported unary RPC.',
    'A unary handler returns a V1-only reply through a legacy-compatible custom codec while binary logging is enabled.',
    'Introduced. The logger import narrows recognition to V2. The custom codec keeps the RPC successful independently of default codec failure.',
    'The response is transmitted but its binary-log payload and length become empty and zero.',
    'Approve the unary response claim routinely. Keep broader logger-code observations separate from claims about request or streaming production traffic.',
    ['server.go constructs ServerMessage with Message: reply.','Saved logger probe changes from 13 bytes to 0 bytes at head.','Both logger type assertions narrow, and the custom codec is a supported independent trigger.'],
    ['Unary requests and streaming log callers pass serialized byte slices, which still match the byte branch.','A failing default codec alone would prevent a successful legacy RPC, so the custom codec qualification matters.'],
    ['The probe constructs a server log entry directly. The pinned production unary caller establishes reachability, but no network RPC or log sink integration was run.'],
    [u+'/change.diff',u+'/base/internal/binarylog/method_logger.go',u+'/head/internal/binarylog/method_logger.go',u+'/head/server.go','probes/grpc-base.txt','probes/grpc-head.txt','probes/grpc-probe.go',str(OUT/'source-evidence/u-grpc-go-6919-head/stream.go')])
add('U-binary-request-overstatement',u,'R010 separately overstates loss of production request payloads','R010','refuted','item-grading',
    'A request-payload allegation must identify an affected production request caller.',
    'R010 says previously logged RPC request contents now disappear because ClientMessage rejects V1 messages.',
    'The V1 assertion narrowing is new, but the relevant production request inputs remain bytes.',
    'The claimed request loss does not occur on the inspected caller paths.',
    'Keep the unary reply recovery in R010. Grade its explicit request-loss assertion separately as refuted. Do not manufacture the same extra allegation from other items that merely describe the narrowed internal assertions.',
    ['The exact phrase in R010 alleges loss of previously logged request and response contents.'],
    ['server.go passes Message: d to ClientMessage.','Client stream SendMsg passes data and server stream RecvMsg passes payInfo.uncompressedBytes.','The byte-slice logger branch remains intact.'],
    ['Only production caller paths relevant to the claim were inspected. Direct internal logger calls with V1 values demonstrate a code fact but do not establish production request loss.'],
    [u+'/head/server.go',u+'/head/internal/binarylog/method_logger.go',str(OUT/'source-evidence/u-grpc-go-6919-head/stream.go')])
add('U-lrs-diagnostic',u,'LRS interval rejection loses its validation diagnostic','R012 R014 R020 R024 R038 R040 R055 R058 R062','advisory','clear',
    'A rejection diagnostic should preserve the cause returned by the validator.',
    'A received LRS response has a missing or protobuf-invalid interval.',
    'Introduced. CheckValid returns an error, but the formatted err is from successful Recv and is nil.',
    'The response is still rejected. The reason is lost, but no failed operational or concrete maintenance task beyond reduced diagnostic detail is established.',
    'Propose advisory. Returning the real error has a specific debugging benefit. The unchanged rejection and absent demonstrated material consequence keep this below the correction threshold on present evidence.',
    ['The pinned branch prints invalid load_reporting_interval: <nil>.','Base formatted the actual ptypes.Duration error.'],
    ['The caller still knows which response field is invalid and retries the stream.','P2 or P3 labels do not prove materiality. No evidence shows a required diagnosis or remediation becomes impossible.'],
    ['Static scope and dataflow suffice to prove the diagnostic bug. No malformed-stream integration or real incident diagnosis was run. A concrete failed maintenance task could justify revisiting materiality.'],
    [u+'/change.diff',u+'/base/xds/internal/xdsclient/transport/loadreport.go',u+'/head/xds/internal/xdsclient/transport/loadreport.go'])
add('U-lrs-positive-overflow',u,'Positive overflowing LRS intervals clamp instead of being rejected','R022','advisory','clear',
    'Preserve the distinction between a protobuf duration and a representable Go duration when validation relies on conversion.',
    'A management response provides a positive duration above the Go time.Duration maximum but inside protobuf bounds.',
    'Introduced acceptance. Base rejects 10000000000 seconds. Head saturates to the maximum duration.',
    'A ticker is accepted with an interval of about 292 years. The claim does not establish a new practical loss of regular reporting relative to already accepted positive intervals just below that bound.',
    'Propose advisory for restoring overflow rejection. The factual conversion difference is clear, but centuries-long intervals below the overflow boundary already prevented regular reports. Do not import the negative-overflow panic into R022.',
    ['Saved duration probe demonstrates positive saturation and loss of conversion error.','recvFirstLoadStatsResponse returns the converted interval to sendLoads.'],
    ['A positive saturated duration does not panic.','Base already accepts very large representable positive intervals and starts a ticker with them.','No new bounded-reporting obligation or reconnect recovery consequence is demonstrated.'],
    ['Conversion and caller source were inspected. No production management-server configuration or useful reporting-frequency expectation for centuries-long values was established.'],
    [u+'/change.diff',u+'/base/xds/internal/xdsclient/transport/loadreport.go',u+'/head/xds/internal/xdsclient/transport/loadreport.go','probes/grpc-probe.go','probes/grpc-base.txt','probes/grpc-head.txt'])
add('U-lrs-negative-overflow',u,'Negative overflowing LRS intervals newly reach a ticker panic','R047','eligible','clear',
    'Reject unrepresentable response durations at the parsing boundary instead of starting a non-positive ticker.',
    'A management response contains Seconds: -10000000000, valid as protobuf data but outside Go duration range.',
    'Introduced for this overflow trigger. Base rejects it before sendLoads. Head accepts and saturates it.',
    'time.NewTicker panics on the newly admitted negative interval, allowing a malformed remote response to crash the load-reporting path.',
    'Approve eligibility routinely for the exact negative-overflow trigger. The pre-existing panic for small negative intervals does not erase this newly admitted input.',
    ['Saved conversion probe returns a base error and a negative saturated head duration.','sendLoads calls time.NewTicker directly.','The ticker probe records non-positive interval for NewTicker.'],
    ['Seconds: -1 was already accepted and panicked on base. That is a different pre-existing trigger.','Negative intervals are not sensible LRS intervals, but remote input rejection is the concrete boundary obligation here.'],
    ['The full remote stream was not executed. The actual conversion and ticker plus pinned caller give a complete static path. No frequency or exploitability estimate is claimed.'],
    [u+'/change.diff',u+'/base/xds/internal/xdsclient/transport/loadreport.go',u+'/head/xds/internal/xdsclient/transport/loadreport.go','probes/grpc-probe.go','probes/grpc-base.txt','probes/grpc-head.txt'])
add('U-rls-overflow',u,'RLS conversion now accepts and clamps unrepresentable durations','R061','advisory','clear',
    'Preserve explicit representability validation when converting service configuration.',
    'An RLS service configuration supplies a protobuf-valid duration outside Go time.Duration range.',
    'Introduced acceptance by convertDuration. Caller-side normalization is otherwise unchanged.',
    'The conversion-level rejection changes. The item establishes no materially different usable configuration behavior or protective failure.',
    'Propose advisory for explicit overflow handling. Assess the actual caller normalization before treating a clamped conversion as a material cache or timeout defect.',
    ['convertDuration now returns AsDuration and CheckValid instead of ptypes.Duration.','The saved duration probe demonstrates the relevant library conversion change.','lookupServiceTimeout is stored and passed to context.WithTimeout.'],
    ['Positive maxAge above five minutes is already normalized to five minutes, so the conversion maximum is not the eventual cache lifetime.','A timeout just inside the Go duration bound was already effectively unbounded in normal operations.','The item alleges acceptance and clamping, not a newly demonstrated panic or missed normal deadline.'],
    ['Inspected parser, duration conversion, maxAge and staleAge normalization, and the timeout consumer. The saved probe runs the conversion libraries, not the full RLS parser. No practical configuration failure is established.'],
    [u+'/change.diff',u+'/base/balancer/rls/config.go',u+'/head/balancer/rls/config.go','probes/grpc-probe.go','probes/grpc-base.txt','probes/grpc-head.txt',str(OUT/'source-evidence/u-grpc-go-6919-head/balancer/rls/control_channel.go')])
add('V-pool-role-recursion',v,'Pool role configuration re-enters its own pool','R005 R025 R027 R029 R035 R044','eligible','clear',
    'Initialize each physical pooled connection with the requested role without checking out another connection from the pool being initialized.',
    'psycopg3 pooling and assume_role are enabled while the first connection is acquired.',
    'Introduced. Configuration moves before the wrapper receives its connection and calls wrapper-bound SQL composition.',
    'Configuration workers cannot finish the initial connections and acquisition times out.',
    'Approve eligibility routinely. Static wrapper-to-cursor-to-ensure_connection-to-pool dataflow and the rerun callback probe support a startup failure for a documented option combination.',
    ['ensure_role uses the supplied raw cursor but composes through ops.compose_sql.','DatabaseOperations.compose_sql calls mogrify using its wrapper, and mogrify opens wrapper.cursor.','The rerun callback reaches the wrapper cursor during raw connection configuration.'],
    ['Normal non-pooled initialization assigns the wrapper connection first and does not have this bootstrap recursion.','The probe stops on wrapper access, so it does not measure a real PoolTimeout.'],
    ['No PostgreSQL server exists. The probe verifies wrapper access rather than actual pool-worker timeout. The caller order establishes the recursion.'],
    [v+'/change.diff',v+'/base/django/db/backends/postgresql/base.py',v+'/head/django/db/backends/postgresql/base.py',v+'/head/django/db/backends/postgresql/operations.py',v+'/head/django/db/backends/postgresql/psycopg_any.py',v+'/head/django/db/backends/base/base.py','probes/django-pool-probe.py',str(OUT/'v-django-17914-head-rerun.txt')])
add('V-pool-test-name',v,'A cached pool retains the original database after test setup switches NAME','R016 R028 R033 R037 R053 R065','eligible','clear',
    'Test database setup must run migrations and tests on the selected test database.',
    'The alias pool is initialized before create_test_db changes NAME, either by earlier access or the postgres-unavailable fallback.',
    'Introduced. close now returns a connection to the pool while the alias cache survives the database-name change.',
    'Subsequent migrations and tests use the original database parameters and can alter original data.',
    'Approve eligibility routinely. The production test lifecycle and alias-only cache demonstrate incorrect database routing. Do not assume every original database is a live production deployment.',
    ['BaseDatabaseCreation closes the wrapper, sets NAME, and calls migrate under the same alias.','The rerun shows wrapper NAME changes to test_prior_database while the identical pool retains prior_database.','The PostgreSQL fallback constructs a wrapper with the same alias and closes only the wrapper.'],
    ['A pool first created after the switch has correct parameters.','Clone and destroy cleanup do not cover the creation switch.','A pre-created pool is a prerequisite, not the default state in every run.'],
    ['No live migrations or PostgreSQL server were run. Actual cached kwargs and inspected migration caller establish routing, not an observed production data modification.'],
    [v+'/change.diff',v+'/head/django/db/backends/postgresql/base.py',v+'/head/django/db/backends/postgresql/creation.py',v+'/head/django/db/backends/base/creation.py','probes/django-pool-probe.py',str(OUT/'v-django-17914-head-rerun.txt')])
add('V-empty-pool-dict',v,'A documented empty pool-options dictionary silently disables pooling','R009 R018 R043 R052 R056 R068','eligible','clear',
    'Accept dictionary pool options, including the empty default-options dictionary promised by the new interface.',
    'A psycopg3 database config sets OPTIONS.pool to an empty dictionary.',
    'New obligation. The pooling interface and documentation are introduced together, but the truthiness guard rejects the documented dictionary form.',
    'Users requesting pooling get direct physical connections instead, losing the selected connection lifecycle and reuse behavior.',
    'Approve eligibility routinely. This is an observable feature configuration failure, not a speculative preference for accepting additional syntax.',
    ['The new docs allow a dictionary passed to ConnectionPool.','True is translated into the same empty dictionary after the guard.','The rerun shows pool_created is false for the empty dictionary.'],
    ['Omitted or False pool values legitimately disable pooling.','The docs show True in the example, but the prose explicitly permits dictionaries without a nonempty restriction.'],
    ['Actual property behavior was rerun. No live connection reuse or resource-pressure workload was measured.'],
    [v+'/change.diff',v+'/head/django/db/backends/postgresql/base.py',v+'/head/docs/ref/databases.txt','probes/django-pool-probe.py',str(OUT/'v-django-17914-head-rerun.txt')])
add('V-psycopg2-doc-contract',v,'The new psycopg2 guidance promises an ignored option but code rejects it','R026','eligible','clear',
    'The documented supported driver configuration must match backend startup behavior.',
    'A deployment with psycopg2 sets a truthy pool option following guidance that the option is ignored.',
    'Introduced documentation and validation disagree in the same change.',
    'A configuration described as harmless instead prevents the connection with ImproperlyConfigured.',
    'Approve eligibility routinely. The correct remedy may be code or documentation, but remedy choice does not create eligibility ambiguity.',
    ['The new docs say the pool option is ignored with psycopg2.','get_connection_params explicitly raises for a truthy pool option when is_psycopg3 is false.','The rerun executes that parameter-extraction branch with the driver flag patched.'],
    ['The actual psycopg2 package and a database server were not used.','An empty dictionary also bypasses this check, but R026 concerns an enabled pool setting.'],
    ['Driver-flag patch verifies the real validation branch. Import-time behavior under an installed psycopg2 environment was not executed, but the static branch is unambiguous.'],
    [v+'/change.diff',v+'/head/django/db/backends/postgresql/base.py',v+'/head/docs/ref/databases.txt','probes/django-pool-probe.py',str(OUT/'v-django-17914-head-rerun.txt')])
add('W-argument-total-order',w,'Canonical argument sorting rejects valid reordered arguments with comparator collisions','R003 R006 R011 R015 R041 R046 R067','eligible','clear',
    'Argument order is insignificant, so otherwise identical field calls with reordered exact-name arguments must merge.',
    'Two valid argument names have large numeric suffixes that collide after JavaScript number accumulation, and field selections reverse their order.',
    'Introduced for argument names. The comparator existed on base for object values, but this change newly uses it to canonicalize argument names.',
    'A valid query accepted by exact-name matching on base now gets a differing-arguments validation error.',
    'Approve the canonical claim routinely. Handle R003 recovery separately because its specific example does not collide.',
    ['Saved base validation accepts the large-suffix query and head rejects it.','The independently rerun exact comparator returns zero for a9007199254740992 and a9007199254740993.','Stable sorting preserves opposite input orders, which print differently.'],
    ['a01a and a1aa compare as -1 and normalize to the same order. R003 uses this incorrect example.','Nested-object sorting already used this comparator, so do not claim all comparator failures are new.'],
    ['The comparator was rerun from exact source with only type annotations removed. Full query validation and performance results were read from hashed saved probes rather than rerun.'],
    [w+'/change.diff',w+'/base/src/validation/rules/OverlappingFieldsCanBeMergedRule.ts',w+'/head/src/validation/rules/OverlappingFieldsCanBeMergedRule.ts',w+'/head/src/jsutils/naturalCompare.ts',w+'/head/src/utilities/sortValueNode.ts','probes/graphql-probe.cjs','probes/graphql-base.txt','probes/graphql-head.txt',str(OUT/'comparator-check.cjs'),str(OUT/'comparator-check.txt')])
add('W-argumentless-perf',w,'Argumentless overlap validation adds expensive serialization to every field pair','R013 R036 R039','eligible','clear',
    'Preserve practical synchronous validation performance for ordinary valid repeated argumentless selections.',
    'A valid query contains many repeated argumentless scalar field selections or overlapping fragments.',
    'Worsened. Pairwise comparisons are already quadratic at base, but head newly allocates, sorts and prints synthetic objects for every pair.',
    'The saved 1000-selection validation median rises from 81.077681 milliseconds to 2703.115921 milliseconds, blocking synchronous validation far longer.',
    'Approve eligibility routinely. The large measured increase, direct code mechanism, and archived bisect-and-revert evidence establish a material regression. Do not equate pre-existing quadratic complexity with this introduced multiplier.',
    ['Three saved samples per revision show a median ratio of 33.3398.','The old sameArguments accepts two empty arrays without serialization.','Archived PR 3958 attributes the degradation to PR 3457 and fixes it.'],
    ['Native item timings differ by machine and field count. R013 reports 3000 fields, which the saved probe did not rerun.','No claim of a full production denial-of-service exploit follows solely from the focused measurement.'],
    ['The focused synthetic valid query was measured previously, not rerun in this candidate. Saved samples are hashed. No production request distribution or remote exploit was measured.'],
    [w+'/change.diff',w+'/base/src/validation/rules/OverlappingFieldsCanBeMergedRule.ts',w+'/head/src/validation/rules/OverlappingFieldsCanBeMergedRule.ts','probes/graphql-probe.cjs','probes/graphql-base.txt','probes/graphql-head.txt','probe-receipt.v1.json','upstream/opus/sources/graphql-js-3958/pr.json'])
add('Y-legacy-user-protocol',y,'Hash mismatch crashes custom users that lack the new fallback method','R001 R002 R008 R031 R045 R048 R049','eligible','clear',
    'Continue flushing stale sessions for supported custom users implementing the existing session-hash protocol.',
    'A custom user implements get_session_auth_hash without inheriting AbstractBaseUser, and a stored session hash becomes stale after a password change.',
    'Introduced. The existing hasattr gate still accepts the old protocol, but a mismatch now unconditionally calls the new method.',
    'Session invalidation becomes an AttributeError and request error even with no fallback secrets configured.',
    'Approve eligibility routinely. This supported custom-user protocol is explicitly documented and the rerun proves the changed invalidation behavior.',
    ['The docs permit an AUTH_USER_MODEL that implements its own get_session_auth_hash.','The rerun base flushes the session and returns anonymous. Head raises AttributeError without flushing.','The empty fallback-secret list does not avoid calling the missing method.'],
    ['Users inheriting AbstractBaseUser get the new method and avoid this missing-method failure.','The fixture is a protocol test double, not a full ORM custom model.'],
    ['Actual get_user source was executed with backend and session test doubles. No live HTTP request or persisted custom ORM user was required to observe the missing-method crash.'],
    [y+'/change.diff',y+'/base/django/contrib/auth/__init__.py',y+'/head/django/contrib/auth/__init__.py',y+'/head/docs/topics/auth/default.txt','probes/django-auth-probe.py',str(OUT/'y-django-16631-base-rerun.txt'),str(OUT/'y-django-16631-head-rerun.txt')])
question='Does the rotation fix create an obligation to retain sessions for existing subclasses that override only the public get_session_auth_hash method, or may those subclasses be required to implement the newly added fallback method to receive rotation support?'
add('Y-custom-hash-rotation',y,'Inherited fallback hashes omit custom public hash overrides','R032 R042','unresolved','needs-human',
    'Potential new obligation to honor supported custom session-hash algorithms during secret rotation.',
    'An AbstractBaseUser subclass overrides get_session_auth_hash to include extra authentication state, inherits the new fallback method, and rotates the secret with the old key in SECRET_KEY_FALLBACKS.',
    'Not a before-to-after regression for this custom user. Both revisions flush it. The candidate depends on whether the new feature promise extends to existing custom public overrides.',
    'If included in the promise, valid custom sessions are logged out during the supported rotation procedure. If extension authors own fallback adaptation, this is an extension requirement instead.',
    'Recommend eligible as an incomplete new promise for a documented public extension point. Ask only the scope question before approval. Keep scope-excluded as the explicit alternative, rather than calling the factual observation false.',
    ['The rerun shows fallback hashes do not match the custom stored hash while default users now survive rotation.','The public override method remains documented.','Upstream discussion preserved the public method signature for compatibility.','The release note and session docs describe avoiding session invalidation through fallback rotation.'],
    ['The custom logout behavior is the same on base and head.','The newly documented fallback method specifically yields the password-field HMAC.','There is no generic safe way to rerun an arbitrary public override with another secret unless a new extension interface is implemented.','No archived maintainer ruling on this exact custom-override promise was found.'],
    ['Actual auth behavior was rerun with a custom hash wrapper. Runtime checks settle the mechanism and base behavior, but cannot decide the supported extension responsibility or intended promise breadth.'],
    [y+'/change.diff',y+'/head/django/contrib/auth/base_user.py',y+'/head/docs/topics/auth/default.txt',y+'/head/docs/ref/settings.txt','probes/django-auth-probe.py',str(OUT/'y-django-16631-base-rerun.txt'),str(OUT/'y-django-16631-head-rerun.txt'),'upstream/sol/sources/django-django-16631/review-comments.json'],question)
lookup={r['id']:r for r in issues}
items=[]
for src in ground['items']:
    token=src['token'];matches=[r for r in issues if token in r['original_tokens']]
    assert matches,token
    main=matches[0]
    route=main['route']
    notes=[]
    if token=='R003':
        route='item-grading'
        notes.append('The general comparator-collision mechanism matches W-argument-total-order, but a01a/a1aa is an incorrect example. Independently rerun comparator result is -1 and the saved query has no error on either revision. Assess whether its general explanation identifies the canonical problem sufficiently for recovery. Do not emit an extra false finding merely for this example.')
    if token=='R010':
        route='item-grading'
        notes.append('This mixed item identifies the valid unary reply problem and independently asserts production request payload loss. Preserve the eligible reply recovery while assessing the request allegation against byte-slice callers.')
    if token in ['R021','R050']:
        route='item-grading'
        notes.append('Both narrowed logger assertions are a true code observation. The concrete reachable defect is unary server response logging. Do not infer that requests or streaming messages lose payloads merely from the shared assertion code or from advice to adapt both branches.')
    if token=='R013':notes.append('The saved probe confirms the same mechanism at 1000 fields, not the particular 3000-field timing reported by the item. Timing precision is ordinary item verification, not canonical eligibility.')
    if token in ['R016','R028','R033','R037','R053','R065']:notes.append('The data-routing defect is material even if the original database is a development database. Potential production modification is conditional and was not executed.')
    if token in ['R012','R014','R020','R024','R038','R040','R055','R058','R062']:notes.append('The native priority does not decide whether diagnostic detail is material. The proposed advisory outcome is pending routine official approval, without a substantive unresolved policy question on the present evidence.')
    items.append(dict(token=token,target=src['target'],original_item=src['item'],canonical_issue_ids=[r['id'] for r in matches],proposed_outcomes=[r['proposed_outcome'] for r in matches],route=route,human_question=main['exact_unresolved_question'],evidence_paths=list(dict.fromkeys(p for r in matches for p in r['evidence_paths'])),reasoning=[main['assessment'],main['recommendation']]+notes,counterevidence=[e for r in matches for e in r['opposing_evidence']],limits=[e for r in matches for e in r['evidence_limits']],administrative_approval_needed=True))
research=[]
def research_add(id,target,summary,outcome,route,obligation,trigger,attribution,consequence,recommendation,support,opp,limits,paths):
    research.append(dict(id=id,target=target,summary=summary,original_tokens=[],proposed_outcome=outcome,route=route,exact_unresolved_question=None,recommendation=recommendation,why=recommendation,supporting_evidence=support,opposing_evidence=opp,evidence_limits=limits,evidence_paths=[p if p.startswith('/') else E+p for p in paths],administrative_approval_needed=True,assessment=dict(concrete_obligation=obligation,reachable_supported_trigger=trigger,attribution=attribution,material_consequence=consequence)))
research_add('H-V-timezone-QuestDB',v,'Connection initialization bypasses a concrete QuestDB timezone override','eligible','clear',
    'Keep subclass-specific initialization dispatch for a PostgreSQL-protocol backend that cannot run PostgreSQL set_config.',
    'A QuestDB-derived wrapper overrides ensure_timezone to avoid unsupported set_config during connection initialization.',
    'Introduced at the exact selected head. The free function replaces virtual dispatch even without pooling enabled.',
    'The concrete derived backend can no longer suppress unsupported timezone SQL and fails initialization.',
    'Approve eligibility routinely. Undocumented status alone does not exclude a concrete supported extension regression confirmed by the exact upstream report.',
    ['The base-to-head rerun shows timezone_called changing from true to false.','Trac 35688 names QuestDB, the unsupported set_config call, and selected commit fad334e.','The report was accepted as a release blocker and repaired in archived PR 18498.'],
    ['The method is explicitly called undocumented in archived release-note discussion.','PostgreSQL itself supports the SQL, so the consequence depends on the concrete derived backend.','This is not the same trigger or failure as pooled assume_role recursion.'],
    ['No QuestDB server was run. Dispatch loss was rerun and the archived reporter supplies the concrete backend consequence. Upstream acceptance is technical evidence, not benchmark human authority.'],
    [v+'/change.diff',v+'/base/django/db/backends/postgresql/base.py',v+'/head/django/db/backends/postgresql/base.py','probes/django-pool-probe.py',str(OUT/'v-django-17914-base-rerun.txt'),str(OUT/'v-django-17914-head-rerun.txt'),'upstream/opus/sources/django-trac/trac-35688.txt','upstream/opus/sources/django-18498/pr.json','upstream/opus/sources/django-18498/review-comments.json'])
research_add('H-V-role-extension',v,'The separately removed role extension method no longer dispatches subclass overrides','advisory','clear',
    'Preserve useful backend customization through an existing role initialization override.',
    'A derived backend overrides ensure_role for its own role setup.',
    'Introduced. The method is removed and configuration directly calls the new free function.',
    'Lost customization is real, but the dossier contains no concrete role-dependent backend failure equivalent to the QuestDB timezone report.',
    'Propose advisory for restoring the extension hook. Keep it separate from timezone and pool role recursion. Do not borrow the unsupported QuestDB timezone SQL as proof of a distinct role consequence.',
    ['The rerun shows role_called changing from true to false.','Archived PR 18498 restores role handling as well.'],
    ['The exact archived discussion says restoring the available-ish role method felt appropriate during timezone restoration and offers to remove it.','No role consumer or required role customization failure is identified in the saved evidence.'],
    ['The dispatch fact was rerun. The concrete benefit is enabling role customization, but material operational harm remains unestablished after inspecting the source and archived rationale. Additional actual downstream evidence could promote this claim.'],
    [v+'/change.diff','probes/django-pool-probe.py',str(OUT/'v-django-17914-base-rerun.txt'),str(OUT/'v-django-17914-head-rerun.txt'),'upstream/opus/sources/django-18498/review-comments.json'])
research_add('H-X-note-limit-drift',x,'A copied scheduler note limit allegedly becomes unsafe when API validation changes','refuted','clear',
    'Scheduler event notes must stay within the API acceptance bound while API owners retain validation authority.',
    'The API bound changes after a local scheduler copy is frozen at 1024 bytes.',
    'The copy is introduced, but scheduler truncation remains behaviorally identical at the selected revision.',
    'No unsafe oversize note follows from a nondecreasing API limit. A local older limit remains conservative.',
    'Reject the unsafe-drift allegation. A request to use future larger notes or add synchronization tests may be advisory, but it is not proof that this local copy causes rejected notes.',
    ['The diff copies the value and retains the same suffix and byte-slice truncation.','API OWNERS reserves approval authority for api-approvers.'],
    ['Archived exact discussion states that the API constant would only expand and the local copy remains safe when shorter.','Moving API validation constants to scheduler helpers or adding a reverse dependency undermines the stated ownership separation.','Possible future additional truncation is different from exceeding the API limit.'],
    ['No Kubernetes build was performed and there are no original saved items for this target in the 68-item input. This assessment concerns only the unsafe-drift hypothesis, not whole-PR cleanliness.'],
    [x+'/change.diff',x+'/base/pkg/scheduler/schedule_one.go',x+'/head/pkg/scheduler/schedule_one.go',x+'/head/pkg/apis/OWNERS',x+'/head/pkg/apis/core/OWNERS','upstream/opus/sources/kubernetes-141463/review-comments.json','upstream/sol/sources/debate-kube141463-apis-OWNERS'])
research_add('H-Y-fallback-timing',y,'Short-circuit fallback checking allegedly creates an authentication timing attack','unsupported','clear',
    'Comparisons must not leak a matching prefix that permits byte-by-byte authentication hash forgery.',
    'A request matches a current or early fallback hash, or fails all configured hashes.',
    'Fallback count variation is new. Early success reveals an overall match class, not a matching prefix inside an HMAC comparison.',
    'Variable work is supported, but no bypass or secret-recovery consequence is established for the asserted security defect.',
    'Do not promote the timing-attack hypothesis. The exact reviewer retracted it after the constant-time comparison distinction was explained. Keep the broader observation about work count without treating the entire PR as clean.',
    ['The pinned any generator stops on a full fallback match.','Hashing overhead differs by the number of keys tested.'],
    ['Each hash comparison uses constant_time_compare, implemented with secrets.compare_digest.','Archived comments distinguish whole-session validity from byte-by-byte guessing and include the explicit retraction.','Authentication and anonymous downstream paths already expose whether the session is valid.'],
    ['No remote timing attack was measured. Adequate inspection of the comparison primitive and exact debated claim provides no prefix oracle or security consequence. A different specified attacker model would need new evidence.'],
    [y+'/change.diff',y+'/head/django/contrib/auth/__init__.py',y+'/head/django/contrib/auth/base_user.py',y+'/head/django/utils/crypto.py',y+'/head/docs/ref/settings.txt','upstream/sol/sources/django-django-16631/review-comments.json'])
human_queue=[dict(id='HQ-custom-hash-promise',canonical_issue_ids=['Y-custom-hash-rotation'],original_tokens=['R032','R042'],pr_summary='Django PR 16631 adds fallback-secret session verification so key rotation can preserve authenticated sessions.',precise_uncertainty=question,alternatives=[dict(outcome='eligible',meaning='The new rotation promise covers existing documented public hash overrides. The inherited fallback implementation must support their algorithm or clearly provide a migration contract.'),dict(outcome='scope-excluded',meaning='Existing custom hash subclasses own implementation of the new fallback method. Their unchanged logout behavior is outside the promised compatibility scope of this patch.')],recommendation='Eligible as an incomplete new promise, subject to the human scope ruling.',reasoning='Runtime behavior and attribution are settled. The public override is documented and the rotation guidance is broad, but base also logs these users out and generic fallback support needs extension cooperation. Only intended supported scope remains uncertain.',evidence_paths=lookup['Y-custom-hash-rotation']['evidence_paths'])]
notes=[
    dict(tokens=['R003'],kind='recovery-and-example',note='The example is refuted, but it is part of a general valid total-order claim. Assess sufficient identification of the comparator-collision problem without adding an extra emitted false finding solely for the wrong example.'),
    dict(tokens=['R010'],kind='mixed-assertions',note='Retain unary reply recovery and separately assess the independently alleged request loss. The byte-slice production caller is direct counterevidence.'),
    dict(tokens=['R021','R050'],kind='claim-precision',note='Both narrowed assertions are real code facts. Do not turn advice to adapt both into a separate production request-loss allegation without exact wording that alleges it.'),
    dict(tokens=['R013','R036','R039'],kind='timing-precision',note='Measurements vary by workload and machine. Canonical performance eligibility is supported by the saved 1000-field median and archived bisect evidence. Exact native numbers need no human conceptual ruling.'),
    dict(tokens=['R022','R047','R061'],kind='distinct-trigger-and-consequence',note='Keep positive LRS saturation, negative LRS panic, and RLS configuration conversion separate. Different consumers and consequences prevent automatic equivalence.'),
    dict(tokens=[r['token'] for r in ground['items']],kind='approval-authority',note='Every proposed eligibility or below-threshold outcome is an automation recommendation. ADR-0002 still requires a saved human ruling before official reference changes or scoring. Administrative approval is not substantive ambiguity.'),
    dict(tokens=[],kind='research-denominator',note='The four research-only hypotheses do not increase the original 68-item denominator. No empty-review or retracted-claim inference establishes whole-PR cleanliness.'),
]
manifest_checks=[]
for rel in ['source-manifest.v1.json','upstream/manifest.v1.json','probe-receipt.v1.json']:
    path=ROOT/E/rel;xdata=json.loads(path.read_text());rows=xdata if isinstance(xdata,list) else xdata['probe_files']
    for row in rows:assert hashlib.sha256((ROOT/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
    manifest_checks.append(dict(path=E+rel,entries=len(rows),result='all hashes match'))
counts=dict(original_items=len(items),canonical_issues=len(issues),research_only=len(research),human_queue=len(human_queue),evidence_queue=0,item_routes=dict(Counter(r['route'] for r in items)),canonical_outcomes=dict(Counter(r['proposed_outcome'] for r in issues)),canonical_routes=dict(Counter(r['route'] for r in issues)),research_outcomes=dict(Counter(r['proposed_outcome'] for r in research)))
audit=dict(schema_version=1,status='Independent proposed triage only. No official decisions, reference edits, scores, or registry edits.',grounding_path='/tmp/arena-selected-pr-triage-2026-09-30/grounding.json',grounding_sha256=hashlib.sha256(Path('/tmp/arena-selected-pr-triage-2026-09-30/grounding.json').read_bytes()).hexdigest(),coverage=counts,items=items,canonical_issues=issues,research_only=research,human_queue=human_queue,evidence_queue=[],ordinary_grading_notes=notes,verification=dict(manifest_checks=manifest_checks,rerun_checks=['Django auth probe on isolated base and head copies, both exited 0.','Django pooling probe on isolated base and head copies, both exited 0.','Exact naturalCompare source with type annotations removed, Node exited 0.'],limits=['No live PostgreSQL or QuestDB server.','gRPC and full GraphQL executions rely on source-inspected, hash-verified saved probes.','No new reviewers, grading sessions, network calls or forge writes.']),administrative_approval=dict(required=True,authority='No human authority was inferred or invented.',rule='ADR-0002 requires saved human rulings for new and disputed outcomes before official scoring. Clear proposals can be approved in a batch; substantive queue entries require a scope choice.'))
OUT.joinpath('audit.json').write_text(json.dumps(audit,indent=2,ensure_ascii=False)+'\n')
print(json.dumps(counts,indent=2))
