/*
Probe P1, part 1. Laid over pkg/scheduler with `go test -overlay`, so that it runs the commit's own
handleSchedulingFailure (and so its own truncateMessage and its own Eventf call). It changes no file of
the clone.

For each scheduling-failure message it:
 1. calls sched.handleSchedulingFailure with an Unschedulable status carrying that message, as the
    scheduler does after a failed attempt; the events recorder is the client-go events.k8s.io/v1
    recorder the scheduler uses, writing to a sink that keeps the event;
 2. prints the message, what truncateMessage returns for it, and the note of the event the recorder built;
 3. for a client content type of JSON and of protobuf: encodes that event as the typed client does,
    decodes it as the API server does, and prints what ValidateEventCreate (the events registry's
    create validation) says;
 4. prints whether the pod's PodScheduled condition, as written to the fake client, holds the message.
No API server runs here; part 2 (apiserver_probe_test.go) does that.
*/
package scheduler

import (
	"bytes"
	"context"
	"fmt"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

	v1 "k8s.io/api/core/v1"
	eventsv1 "k8s.io/api/events/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/runtime/schema"
	"k8s.io/client-go/informers"
	clientsetfake "k8s.io/client-go/kubernetes/fake"
	clientscheme "k8s.io/client-go/kubernetes/scheme"
	"k8s.io/client-go/tools/events"
	"k8s.io/klog/v2/ktesting"
	fwk "k8s.io/kube-scheduler/framework"
	"k8s.io/kubernetes/pkg/api/legacyscheme"
	api "k8s.io/kubernetes/pkg/apis/core"
	_ "k8s.io/kubernetes/pkg/apis/core/install"
	"k8s.io/kubernetes/pkg/apis/core/validation"
	_ "k8s.io/kubernetes/pkg/apis/events/install"
	internalqueue "k8s.io/kubernetes/pkg/scheduler/backend/queue"
	"k8s.io/kubernetes/pkg/scheduler/framework"
	"k8s.io/kubernetes/pkg/scheduler/framework/plugins/defaultbinder"
	"k8s.io/kubernetes/pkg/scheduler/framework/plugins/queuesort"
	frameworkruntime "k8s.io/kubernetes/pkg/scheduler/framework/runtime"
	"k8s.io/kubernetes/pkg/scheduler/metrics"
	st "k8s.io/kubernetes/pkg/scheduler/testing"
	tf "k8s.io/kubernetes/pkg/scheduler/testing/framework"
)

type p1Sink struct{ got chan *eventsv1.Event }

func (s *p1Sink) Create(_ context.Context, e *eventsv1.Event) (*eventsv1.Event, error) {
	s.got <- e.DeepCopy()
	return e, nil
}
func (s *p1Sink) Update(_ context.Context, e *eventsv1.Event) (*eventsv1.Event, error) { return e, nil }
func (s *p1Sink) Patch(_ context.Context, e *eventsv1.Event, _ []byte) (*eventsv1.Event, error) {
	return e, nil
}

// p1Pad returns head, then filler up to total bytes, then tail.
func p1Pad(head string, total int, tail string) string {
	return head + strings.Repeat("a", total-len(head)-len(tail)) + tail
}

// p1At returns total bytes of filler with s placed at byte offset start.
func p1At(start int, s string, total int) string {
	return strings.Repeat("a", start) + s + strings.Repeat("b", total-start-len(s))
}

func p1Excerpt(s string) string {
	i := strings.Index(s, "%")
	if i < 0 {
		return ""
	}
	lo, hi := i-12, i+28
	if lo < 0 {
		lo = 0
	}
	if hi > len(s) {
		hi = len(s)
	}
	return strings.ToValidUTF8(s[lo:hi], "?")
}

func p1Tail(s string) string {
	r := []rune(strings.ToValidUTF8(s, "�"))
	if len(r) > 10 {
		r = r[len(r)-10:]
	}
	return string(r)
}

func TestP1Chain(t *testing.T) {
	sentence := "节点资源不足，" // seven 3-byte characters
	cases := []struct{ group, name, msg string }{
		{"control", "ascii-short", "0/3 nodes are available: 3 node(s) had insufficient cpu."},
		{"control", "ascii-1100", strings.Repeat("a", 1100)},
		{"percent", "percent-short-80-of", "node cpu usage is above 80% of allocatable"},
		{"percent", "percent-short-verb-s", "extender rejected the pod: quota %s exceeded"},
		{"percent", "percent-short-trailing", "load is 95%"},
		{"percent", "percent-short-doubled", "load is 95%% of capacity"},
		{"percent", "percent-cel-modulo-short", `selector #0: device.attributes["dra.example.com"].index % 2 == 0 did not match`},
		{"percent", "percent-1100-near-start", p1Pad("node usage above 80% of allocatable; ", 1100, "")},
		{"percent", "percent-1100-three-near-start", p1Pad("cpu 91% of node, memory 87% of node, disk 99% of node; ", 1100, "")},
		{"percent", "percent-1020-not-truncated", p1Pad("node usage above 80% of allocatable; ", 1020, "")},
		{"percent", "percent-1000-not-truncated", p1Pad("node usage above 80% of allocatable; ", 1000, "")},
		{"percent", "percent-1024-five-not-truncated", p1Pad("a 10% b 20% c 30% d 40% e 50% f; ", 1024, "")},
		{"percent", "percent-after-cut-1100", p1At(1050, "80% of", 1100)},
		{"percent", "percent-at-1019", p1At(1019, "%", 1100)},
		{"multibyte", "2-byte-char-at-1019", p1At(1019, "é", 1100)},
		{"multibyte", "3-byte-char-at-1019", p1At(1019, "节", 1100)},
		{"multibyte", "3-byte-char-at-1018", p1At(1018, "节", 1100)},
		{"multibyte", "4-byte-char-at-1017", p1At(1017, "\U0001F600", 1100)},
		{"multibyte", "cjk-text-after-28-byte-ascii-prefix", "0/5 nodes are available: 5 x" + strings.Repeat(sentence, 60)},
		{"multibyte", "cjk-1025-bytes", "ab" + strings.Repeat("节", 341)},
		{"both", "percent-near-start-and-3-byte-char-at-1019", "usage 80% of node; " + p1At(1019-len("usage 80% of node; "), "节", 1100-len("usage 80% of node; "))},
	}

	logger, ctx := ktesting.NewTestContext(t)
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	internalEvents := schema.GroupVersion{Group: "events.k8s.io", Version: runtime.APIVersionInternal}

	for i, c := range cases {
		pod := st.MakePod().Name(fmt.Sprintf("p1-%d", i)).Namespace("ns").UID(fmt.Sprintf("uid-%d", i)).SchedulerName("default-scheduler").Obj()
		client := clientsetfake.NewClientset(pod)
		informerFactory := informers.NewSharedInformerFactory(client, 0)
		sink := &p1Sink{got: make(chan *eventsv1.Event, 4)}
		broadcaster := events.NewBroadcaster(sink)
		broadcaster.StartRecordingToSink(ctx.Done())

		schedFramework, err := tf.NewFramework(ctx,
			[]tf.RegisterPluginFunc{
				tf.RegisterQueueSortPlugin(queuesort.Name, queuesort.New),
				tf.RegisterBindPlugin(defaultbinder.Name, defaultbinder.New),
			},
			"default-scheduler",
			frameworkruntime.WithClientSet(client),
			frameworkruntime.WithEventRecorder(broadcaster.NewRecorder(clientscheme.Scheme, "default-scheduler")),
			frameworkruntime.WithInformerFactory(informerFactory),
		)
		if err != nil {
			t.Fatal(err)
		}
		ar := metrics.NewMetricsAsyncRecorder(10, time.Second, ctx.Done())
		queue := internalqueue.NewSchedulingQueue(nil, informerFactory, internalqueue.WithMetricsRecorder(ar))
		sched := &Scheduler{client: client, SchedulingQueue: queue}
		informerFactory.Start(ctx.Done())
		informerFactory.WaitForCacheSync(ctx.Done())
		queue.Add(ctx, pod)
		popped, err := queue.Pop(logger)
		if err != nil {
			t.Fatalf("Pop: %v", err)
		}

		sched.handleSchedulingFailure(ctx, schedFramework, popped.(*framework.QueuedPodInfo), fwk.NewStatus(fwk.Unschedulable, c.msg), nil, time.Now())

		var ev *eventsv1.Event
		select {
		case ev = <-sink.got:
		case <-time.After(10 * time.Second):
			t.Fatalf("%s: the recorder delivered no event", c.name)
		}
		broadcaster.Shutdown()

		trunc := truncateMessage(c.msg)
		fmt.Printf("CASE %s (%s)\n", c.name, c.group)
		fmt.Printf("  message: bytes=%d valid_utf8=%v percent_signs=%d\n", len(c.msg), utf8.ValidString(c.msg), strings.Count(c.msg, "%"))
		fmt.Printf("  truncateMessage: bytes=%d valid_utf8=%v\n", len(trunc), utf8.ValidString(trunc))
		fmt.Printf("  event note: bytes=%d valid_utf8=%v equals_truncateMessage=%v contains_%%!=%v\n",
			len(ev.Note), utf8.ValidString(ev.Note), ev.Note == trunc, strings.Contains(ev.Note, "%!"))
		if ex := p1Excerpt(c.msg); ex != "" {
			fmt.Printf("    message around first %%: %q\n", ex)
			fmt.Printf("    note around first %%:    %q\n", p1Excerpt(ev.Note))
		}
		if len(ev.Note) > 200 {
			fmt.Printf("    note ends: %q\n", p1Tail(ev.Note))
		}

		for _, mediaType := range []string{runtime.ContentTypeJSON, runtime.ContentTypeProtobuf} {
			clientInfo, ok := runtime.SerializerInfoForMediaType(clientscheme.Codecs.SupportedMediaTypes(), mediaType)
			if !ok {
				t.Fatalf("no client serializer for %s", mediaType)
			}
			var body bytes.Buffer
			if err := clientscheme.Codecs.WithoutConversion().EncoderForVersion(clientInfo.Serializer, eventsv1.SchemeGroupVersion).Encode(ev, &body); err != nil {
				fmt.Printf("  %s: client encode error: %v\n", mediaType, err)
				continue
			}
			serverInfo, ok := runtime.SerializerInfoForMediaType(legacyscheme.Codecs.SupportedMediaTypes(), mediaType)
			if !ok {
				t.Fatalf("no server serializer for %s", mediaType)
			}
			obj, _, err := legacyscheme.Codecs.DecoderToVersion(serverInfo.Serializer, internalEvents).Decode(body.Bytes(), nil, nil)
			if err != nil {
				fmt.Printf("  %s: server decode error: %v\n", mediaType, err)
				continue
			}
			internal := obj.(*api.Event)
			errs := validation.ValidateEventCreate(internal, eventsv1.SchemeGroupVersion)
			verdict := "ACCEPTED"
			if len(errs) > 0 {
				verdict = "REJECTED: " + errs.ToAggregate().Error()
			}
			fmt.Printf("  %s: decoded_message_bytes=%d decoded_valid_utf8=%v validation=%s\n",
				mediaType, len(internal.Message), utf8.ValidString(internal.Message), verdict)
		}

		got, err := client.CoreV1().Pods("ns").Get(ctx, pod.Name, metav1.GetOptions{})
		if err != nil {
			t.Fatalf("get pod: %v", err)
		}
		for _, cond := range got.Status.Conditions {
			if cond.Type == v1.PodScheduled {
				fmt.Printf("  PodScheduled condition: status=%s reason=%s message_equals_input=%v\n", cond.Status, cond.Reason, cond.Message == c.msg)
			}
		}
	}
}
