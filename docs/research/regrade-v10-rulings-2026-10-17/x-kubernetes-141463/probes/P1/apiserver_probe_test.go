/*
Probe P1, part 2. Laid over test/integration/scheduler/queueing with `go test -overlay`. It changes
no file of the clone. Adapted from the earlier probe of the same pull request (M1, part 2).

It starts the repository's integration API server with etcd and the real scheduler of the commit
under test, with one out-of-tree Filter plugin that rejects every node with a message chosen by the
pod's name. It does this twice: once with the scheduler's client content type set to
application/json and once with application/vnd.kubernetes.protobuf, the kube-scheduler default.
For each pod it prints what the scheduler's client posted to the events API and what the server
answered, the FailedScheduling events a JSON reader then finds (with the text around the first '%'),
and the pod's PodScheduled condition.
*/
package queueing

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"
	"unicode/utf8"

	v1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/client-go/kubernetes"
	clientscheme "k8s.io/client-go/kubernetes/scheme"
	restclient "k8s.io/client-go/rest"
	configv1 "k8s.io/kube-scheduler/config/v1"
	fwk "k8s.io/kube-scheduler/framework"
	"k8s.io/kubernetes/pkg/scheduler"
	configtesting "k8s.io/kubernetes/pkg/scheduler/apis/config/testing"
	frameworkruntime "k8s.io/kubernetes/pkg/scheduler/framework/runtime"
	st "k8s.io/kubernetes/pkg/scheduler/testing"
	testutils "k8s.io/kubernetes/test/integration/util"
	imageutils "k8s.io/kubernetes/test/utils/image"
	"k8s.io/utils/ptr"
)

const m1Sentence = "节点资源不足，" // seven 3-byte characters

// The scheduler wraps a plugin's message as "0/1 nodes are available: 1 " + message + ". preemption: ..."
// (27 bytes before it and 86 after it here); the probe prints the condition's length, which is that whole text.
func p1Pad(head string, total int) string { return head + strings.Repeat("a", total-len(head)) }

var m1Messages = map[string]string{
	"p1-ascii-long":     strings.Repeat("insufficient capacity on this node, ", 40),
	"p1-pct-short":      "node cpu usage is above 80% of allocatable",
	"p1-pct-long":       p1Pad("node usage above 80% of allocatable; ", 1440),
	"p1-pct-total-1020": p1Pad("node usage above 80% of allocatable; ", 1020-27-86),
	"p1-pct-total-1000": p1Pad("node usage above 80% of allocatable; ", 1000-27-86),
	"p1-cjk-long-p1":    "x" + strings.Repeat(m1Sentence, 60),
	"p1-terminus-form":  "", // built per node in Filter
}

var m1Order = []string{"p1-ascii-long", "p1-pct-short", "p1-pct-long", "p1-pct-total-1020", "p1-pct-total-1000", "p1-cjk-long-p1"}

type m1Plugin struct{}

func (*m1Plugin) Name() string { return "P1LongMessage" }

func (*m1Plugin) Filter(_ context.Context, _ fwk.CycleState, pod *v1.Pod, nodeInfo fwk.NodeInfo) *fwk.Status {
	if pod.Name == "p1-terminus-form" {
		// The message form of a public out-of-tree Filter plugin (terminus-io/Terminus, pkg/scheduler/scheduler.go):
		// fmt.Sprintf("Insufficient Physical storage: used %d > limit %d (95%%)", requestBytes, free)
		// with a different free size on each node.
		n, _ := strconv.Atoi(strings.TrimPrefix(nodeInfo.Node().Name, "p1-node-"))
		return fwk.NewStatus(fwk.Unschedulable, fmt.Sprintf("Insufficient Physical storage: used %d > limit %d (95%%)", int64(1073741824), int64(5000000000+n*7919)))
	}
	return fwk.NewStatus(fwk.Unschedulable, m1Messages[pod.Name])
}

type m1Post struct {
	contentType string
	bodyBytes   int
	status      int
	answer      string
}

func m1End(s string) string {
	if r := []rune(s); len(r) > 10 {
		return string(r[len(r)-10:])
	}
	return s
}

func TestP1APIServer(t *testing.T) {
	for _, contentType := range []string{runtime.ContentTypeJSON, runtime.ContentTypeProtobuf} {
		t.Run(contentType, func(t *testing.T) { m1Run(t, contentType, 1, append(m1Order, "p1-terminus-form")) })
	}
}

// TestP1ManyNodes: the same, with 16 nodes, each refused with its own text by the public plugin's message form.
func TestP1ManyNodes(t *testing.T) {
	for _, contentType := range []string{runtime.ContentTypeJSON, runtime.ContentTypeProtobuf} {
		t.Run(contentType, func(t *testing.T) { m1Run(t, contentType, 16, []string{"p1-terminus-form"}) })
	}
}

func m1Run(t *testing.T, contentType string, nodes int, order []string) {
	testCtx := testutils.InitTestAPIServer(t, "p1", nil)
	fmt.Printf("CONTENT TYPE %s, NODES %d\n", contentType, nodes)
	fmt.Printf("  the integration framework's own client config has ContentType=%q\n", testCtx.KubeConfig.ContentType)

	readerConfig := restclient.CopyConfig(testCtx.KubeConfig)
	readerConfig.ContentType = runtime.ContentTypeJSON
	reader := kubernetes.NewForConfigOrDie(readerConfig)

	// The scheduler's clients, built the way cmd/kube-scheduler builds them from clientConnection.contentType.
	testCtx.KubeConfig.ContentType = contentType
	testCtx.ClientSet = kubernetes.NewForConfigOrDie(testCtx.KubeConfig)

	var mu sync.Mutex
	posts := map[string][]m1Post{}
	wrapper := testutils.RoundTripWrapper(func(transport http.RoundTripper, req *http.Request) (*http.Response, error) {
		if req.Method != http.MethodPost || !strings.HasSuffix(req.URL.Path, "/events") || req.GetBody == nil {
			return transport.RoundTrip(req)
		}
		bodyReader, _ := req.GetBody()
		body, _ := io.ReadAll(bodyReader)
		resp, err := transport.RoundTrip(req)
		if err != nil {
			return resp, err
		}
		post := m1Post{contentType: req.Header.Get("Content-Type"), bodyBytes: len(body), status: resp.StatusCode}
		if resp.StatusCode >= 300 {
			answer, _ := io.ReadAll(resp.Body)
			resp.Body = io.NopCloser(strings.NewReader(string(answer)))
			var status metav1.Status
			if json.Unmarshal(answer, &status) == nil && status.Message != "" {
				post.answer = status.Message
			} else if obj, _, err := clientscheme.Codecs.UniversalDeserializer().Decode(answer, nil, nil); err == nil {
				if st, ok := obj.(*metav1.Status); ok {
					post.answer = st.Message + " (protobuf Status body)"
				}
			} else {
				post.answer = strings.ToValidUTF8(string(answer), "?")
			}
		}
		mu.Lock()
		defer mu.Unlock()
		for name := range m1Messages {
			if strings.Contains(string(body), name+".") {
				posts[name] = append(posts[name], post)
			}
		}
		return resp, nil
	})
	testCtx.RoundTrip.Store(&wrapper)

	registry := frameworkruntime.Registry{
		"P1LongMessage": func(_ context.Context, _ runtime.Object, _ fwk.Handle) (fwk.Plugin, error) {
			return &m1Plugin{}, nil
		},
	}
	cfg := configtesting.V1ToInternalWithDefaults(t, configv1.KubeSchedulerConfiguration{
		Profiles: []configv1.KubeSchedulerProfile{{
			SchedulerName: ptr.To(v1.DefaultSchedulerName),
			Plugins: &configv1.Plugins{
				Filter: configv1.PluginSet{Enabled: []configv1.Plugin{{Name: "P1LongMessage"}}},
			},
		}}})
	testCtx = testutils.InitTestSchedulerWithOptions(t, testCtx, 0,
		scheduler.WithProfiles(cfg.Profiles...),
		scheduler.WithFrameworkOutOfTreeRegistry(registry),
	)
	testutils.SyncSchedulerInformerFactory(testCtx)
	go testCtx.Scheduler.Run(testCtx.SchedulerCtx)

	cs, ns, ctx := testCtx.ClientSet, testCtx.NS.Name, testCtx.Ctx
	for n := 0; n < nodes; n++ {
		if _, err := cs.CoreV1().Nodes().Create(ctx, st.MakeNode().Name(fmt.Sprintf("p1-node-%d", n)).Obj(), metav1.CreateOptions{}); err != nil {
			t.Fatalf("create node: %v", err)
		}
	}

	for _, name := range order {
		msg := m1Messages[name]
		pod := st.MakePod().Namespace(ns).Name(name).Container(imageutils.GetPauseImageName()).Obj()
		if _, err := cs.CoreV1().Pods(ns).Create(ctx, pod, metav1.CreateOptions{}); err != nil {
			t.Fatalf("create pod %s: %v", name, err)
		}
		if err := testutils.WaitForPodUnschedulable(ctx, reader, pod); err != nil {
			t.Fatalf("pod %s did not become unschedulable: %v", name, err)
		}
		// Give the asynchronous event recorder time to post.
		deadline := time.Now().Add(15 * time.Second)
		for time.Now().Before(deadline) {
			mu.Lock()
			n := len(posts[name])
			mu.Unlock()
			if n > 0 {
				break
			}
			time.Sleep(100 * time.Millisecond)
		}
		time.Sleep(500 * time.Millisecond)

		fmt.Printf("POD %s\n", name)
		if name != "p1-terminus-form" {
			fmt.Printf("  plugin message: bytes=%d valid_utf8=%v\n", len(msg), utf8.ValidString(msg))
		} else {
			fmt.Printf("  plugin message: the public plugin's form, one text per node\n")
			msg = "(95%)"
		}

		got, err := reader.CoreV1().Pods(ns).Get(ctx, name, metav1.GetOptions{})
		if err != nil {
			t.Fatalf("get pod %s: %v", name, err)
		}
		for _, c := range got.Status.Conditions {
			if c.Type == v1.PodScheduled {
				fmt.Printf("  PodScheduled condition: status=%s reason=%s message_bytes=%d valid_utf8=%v replacement_chars=%d holds_whole_plugin_message=%v message_start=%q\n",
					c.Status, c.Reason, len(c.Message), utf8.ValidString(c.Message), strings.Count(c.Message, "�"), strings.Contains(c.Message, msg), c.Message[:27])
			}
		}

		mu.Lock()
		seen := append([]m1Post(nil), posts[name]...)
		mu.Unlock()
		fmt.Printf("  event posts by the scheduler for this pod: %d\n", len(seen))
		for i, p := range seen {
			if i == 2 {
				fmt.Printf("    ... %d more\n", len(seen)-2)
				break
			}
			fmt.Printf("    POST events: request Content-Type=%s -> HTTP %d %s\n", p.contentType, p.status, p.answer)
		}

		list, err := reader.EventsV1().Events(ns).List(ctx, metav1.ListOptions{})
		if err != nil {
			t.Fatalf("list events: %v", err)
		}
		found := 0
		for _, e := range list.Items {
			if e.Regarding.Name != name || e.Reason != "FailedScheduling" {
				continue
			}
			found++
			fmt.Printf("  stored event read as JSON through events.k8s.io/v1: note_bytes=%d valid_utf8=%v replacement_chars=%d contains_%%!=%v note_end=%q\n",
				len(e.Note), utf8.ValidString(e.Note), strings.Count(e.Note, "�"), strings.Contains(e.Note, "%!"), m1End(e.Note))
			if i := strings.Index(e.Note, "%"); i >= 0 {
				fmt.Printf("    note around first %%: %q\n", e.Note[i-12:i+24])
			}
		}
		coreList, err := reader.CoreV1().Events(ns).List(ctx, metav1.ListOptions{FieldSelector: "involvedObject.name=" + name})
		if err != nil {
			t.Fatalf("list core events: %v", err)
		}
		fmt.Printf("  FailedScheduling events found for this pod: events.k8s.io/v1=%d core/v1=%d\n", found, len(coreList.Items))
	}
}
