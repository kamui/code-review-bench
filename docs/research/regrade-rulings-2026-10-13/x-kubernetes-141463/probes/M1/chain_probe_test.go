/*
Probe M1, part 1. Laid over pkg/scheduler with `go test -overlay`, so that it can call the
unexported truncateMessage of the commit under test. It changes no file of the clone.

For each message it prints, using only the project's own code at that commit:
 1. what truncateMessage returns;
 2. the Event the client-go events recorder builds from that note (the recorder the scheduler uses);
 3. for a client content type of JSON and of protobuf: the request body the typed client's
    serializer writes, the object the API server's serializer decodes from it, and what
    ValidateEventCreate (the event registry's create validation) says about that object;
 4. for an accepted object: the note after a protobuf round trip at the storage version and as a
    JSON reader receives it.
No API server runs here; part 2 (apiserver_probe_test.go) does that.
*/
package scheduler

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

	corev1 "k8s.io/api/core/v1"
	eventsv1 "k8s.io/api/events/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/runtime/schema"
	clientscheme "k8s.io/client-go/kubernetes/scheme"
	"k8s.io/client-go/tools/events"
	"k8s.io/kubernetes/pkg/api/legacyscheme"
	api "k8s.io/kubernetes/pkg/apis/core"
	_ "k8s.io/kubernetes/pkg/apis/core/install"
	"k8s.io/kubernetes/pkg/apis/core/validation"
	_ "k8s.io/kubernetes/pkg/apis/events/install"
)

type m1Sink struct{ got chan *eventsv1.Event }

func (s *m1Sink) Create(_ context.Context, e *eventsv1.Event) (*eventsv1.Event, error) {
	s.got <- e.DeepCopy()
	return e, nil
}
func (s *m1Sink) Update(_ context.Context, e *eventsv1.Event) (*eventsv1.Event, error) { return e, nil }
func (s *m1Sink) Patch(_ context.Context, e *eventsv1.Event, _ []byte) (*eventsv1.Event, error) {
	return e, nil
}

// m1At returns a message of total bytes whose only non-ASCII character starts at byte offset start.
func m1At(start int, char string, total int) string {
	return strings.Repeat("a", start) + char + strings.Repeat("b", total-start-len(char))
}

func m1Tail(s string) string {
	if len(s) < 12 {
		return fmt.Sprintf("% x", s)
	}
	return fmt.Sprintf("% x", s[len(s)-12:])
}

func TestM1Chain(t *testing.T) {
	sentence := "\u8282\u70b9\u8d44\u6e90\u4e0d\u8db3\uff0c" // seven 3-byte characters
	cases := []struct{ name, msg string }{
		{"ascii-1100", strings.Repeat("a", 1100)},
		{"2-byte-char-at-1019", m1At(1019, "é", 1100)},
		{"3-byte-char-at-1019", m1At(1019, "节", 1100)},
		{"3-byte-char-at-1018", m1At(1018, "节", 1100)},
		{"4-byte-char-at-1019", m1At(1019, "\U0001F600", 1100)},
		{"4-byte-char-at-1018", m1At(1018, "\U0001F600", 1100)},
		{"4-byte-char-at-1017", m1At(1017, "\U0001F600", 1100)},
		{"3-byte-char-ends-at-1020", m1At(1017, "节", 1100)},
		{"3-byte-char-starts-at-1020", m1At(1020, "节", 1100)},
		{"cjk-text-after-27-byte-ascii-prefix", "0/5 nodes are available: 5 " + strings.Repeat(sentence, 60)},
		{"cjk-text-after-28-byte-ascii-prefix", "0/5 nodes are available: 5 x" + strings.Repeat(sentence, 60)},
		{"cjk-text-after-29-byte-ascii-prefix", "0/12 nodes are available: 12 " + strings.Repeat(sentence, 60)},
		{"cjk-short-1023-bytes", strings.Repeat("节", 341)},
		{"cjk-exactly-1024-bytes", "a" + strings.Repeat("节", 341)},
		{"cjk-1025-bytes", "ab" + strings.Repeat("节", 341)},
	}

	sink := &m1Sink{got: make(chan *eventsv1.Event, 1)}
	broadcaster := events.NewBroadcaster(sink)
	stop := make(chan struct{})
	defer close(stop)
	broadcaster.StartRecordingToSink(stop)
	recorder := broadcaster.NewRecorder(clientscheme.Scheme, "default-scheduler")

	internalEvents := schema.GroupVersion{Group: "events.k8s.io", Version: runtime.APIVersionInternal}

	for i, c := range cases {
		out := truncateMessage(c.msg)
		fmt.Printf("CASE %s\n", c.name)
		fmt.Printf("  truncateMessage: in_bytes=%d in_valid_utf8=%v out_bytes=%d out_valid_utf8=%v changed=%v out_last_12_bytes=[%s]\n",
			len(c.msg), utf8.ValidString(c.msg), len(out), utf8.ValidString(out), out != c.msg, m1Tail(out))

		pod := &corev1.Pod{ObjectMeta: metav1.ObjectMeta{Name: fmt.Sprintf("pod-%d", i), Namespace: "default", UID: "uid"}}
		// The same call as schedule_one.go makes: the note is the format string and there are no arguments.
		recorder.Eventf(pod, nil, corev1.EventTypeWarning, "FailedScheduling", "Scheduling", out)
		var ev *eventsv1.Event
		select {
		case ev = <-sink.got:
		case <-time.After(10 * time.Second):
			t.Fatalf("%s: the recorder delivered no event", c.name)
		}
		fmt.Printf("  recorder event: note_bytes=%d note_valid_utf8=%v note_equals_truncated=%v\n", len(ev.Note), utf8.ValidString(ev.Note), ev.Note == out)

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
				fmt.Printf("  %s: body_bytes=%d server decode error: %v\n", mediaType, body.Len(), err)
				continue
			}
			internal, ok := obj.(*api.Event)
			if !ok {
				t.Fatalf("decoded %T", obj)
			}
			errs := validation.ValidateEventCreate(internal, eventsv1.SchemeGroupVersion)
			verdict := "ACCEPTED"
			if len(errs) > 0 {
				verdict = "REJECTED: " + errs.ToAggregate().Error()
			}
			fmt.Printf("  %s: body_bytes=%d decoded_message_bytes=%d decoded_valid_utf8=%v replacement_chars=%d validation=%s\n",
				mediaType, body.Len(), len(internal.Message), utf8.ValidString(internal.Message), strings.Count(internal.Message, "�"), verdict)
			if len(errs) > 0 {
				continue
			}

			// Stored as protobuf at core/v1, then read back by a JSON client through events.k8s.io/v1.
			protoInfo, _ := runtime.SerializerInfoForMediaType(legacyscheme.Codecs.SupportedMediaTypes(), runtime.ContentTypeProtobuf)
			var stored bytes.Buffer
			if err := legacyscheme.Codecs.EncoderForVersion(protoInfo.Serializer, corev1.SchemeGroupVersion).Encode(internal, &stored); err != nil {
				fmt.Printf("    storage encode error: %v\n", err)
				continue
			}
			back, _, err := legacyscheme.Codecs.DecoderToVersion(protoInfo.Serializer, api.SchemeGroupVersion).Decode(stored.Bytes(), nil, nil)
			if err != nil {
				fmt.Printf("    storage decode error: %v\n", err)
				continue
			}
			jsonInfo, _ := runtime.SerializerInfoForMediaType(legacyscheme.Codecs.SupportedMediaTypes(), runtime.ContentTypeJSON)
			var served bytes.Buffer
			if err := legacyscheme.Codecs.EncoderForVersion(jsonInfo.Serializer, eventsv1.SchemeGroupVersion).Encode(back, &served); err != nil {
				fmt.Printf("    JSON response encode error: %v\n", err)
				continue
			}
			var reader struct {
				Note string `json:"note"`
			}
			if err := json.Unmarshal(served.Bytes(), &reader); err != nil {
				fmt.Printf("    JSON reader error: %v\n", err)
				continue
			}
			end := reader.Note
			if r := []rune(end); len(r) > 8 {
				end = string(r[len(r)-8:])
			}
			fmt.Printf("    stored then read as JSON: stored_message_valid_utf8=%v reader_note_bytes=%d reader_replacement_chars=%d reader_note_end=%q\n",
				utf8.ValidString(back.(*api.Event).Message), len(reader.Note), strings.Count(reader.Note, "�"), end)
		}
	}
}
