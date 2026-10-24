// Copyright (C) 2026, Lux Industries Inc. All rights reserved.
// See the file LICENSE for licensing terms.

package filer

import (
	"bytes"
	"testing"

	filer_pb "github.com/hanzoai/s3/s3/pb/filer_pb"
)

// TestEntryRoundTrip proves the proto→ZAP→proto Entry conversion is lossless
// across the full nested graph (FuseAttributes, []FileChunk with FileId,
// extended map, RemoteEntry). This is the parity gate for the filer gRPC→ZAP
// rip: callsites that read/write Entry over ZAP must see byte-identical data.
func TestEntryRoundTrip(t *testing.T) {
	orig := &filer_pb.Entry{
		Name:            "team-go/blob.bin",
		IsDirectory:     false,
		HardLinkId:      []byte{0xde, 0xad},
		HardLinkCounter: 3,
		Content:         []byte("inline object content"),
		Quota:           1 << 20,
		Attributes: &filer_pb.FuseAttributes{
			FileSize:      4096,
			Mtime:         1700000000,
			FileMode:      0o644,
			Uid:           1000,
			Gid:           1000,
			Crtime:        1699999999,
			Mime:          "application/octet-stream",
			TtlSec:        3600,
			UserName:      "hanzo",
			GroupName:     []string{"staff", "ai"},
			SymlinkTarget: "",
			Md5:           []byte{1, 2, 3, 4},
			Inode:         42,
			Atime:         1700000001,
		},
		Chunks: []*filer_pb.FileChunk{
			{
				FileId:       "3,01637037d6",
				Offset:       0,
				Size:         4096,
				ModifiedTsNs: 1700000000000000000,
				ETag:         "abc123",
				Fid:          &filer_pb.FileId{VolumeId: 3, FileKey: 0x1637037d6, Cookie: 0xff},
				CipherKey:    []byte{9, 9, 9},
				IsCompressed: true,
				SseType:      1,
			},
		},
		Extended: map[string][]byte{
			"Content-Type": []byte("application/octet-stream"),
			"ETag":         []byte("abc123"),
		},
		RemoteEntry: &filer_pb.RemoteEntry{
			StorageName:       "s3-backend",
			LastLocalSyncTsNs: 1700000000000000001,
			RemoteETag:        "remote-etag",
			RemoteMtime:       1699999998,
			RemoteSize:        4096,
		},
	}

	got, err := EntryFromWire(EntryToWire(orig))
	if err != nil {
		t.Fatalf("EntryFromWire: %v", err)
	}

	if got.Name != orig.Name || got.IsDirectory != orig.IsDirectory ||
		got.HardLinkCounter != orig.HardLinkCounter || got.Quota != orig.Quota ||
		!bytes.Equal(got.HardLinkId, orig.HardLinkId) || !bytes.Equal(got.Content, orig.Content) {
		t.Fatalf("scalar Entry fields diverged:\n got=%+v\nwant=%+v", got, orig)
	}

	// Attributes
	ga, oa := got.Attributes, orig.Attributes
	if ga == nil {
		t.Fatal("attributes dropped")
	}
	if ga.FileSize != oa.FileSize || ga.Mtime != oa.Mtime || ga.FileMode != oa.FileMode ||
		ga.Uid != oa.Uid || ga.Gid != oa.Gid || ga.Mime != oa.Mime || ga.TtlSec != oa.TtlSec ||
		ga.UserName != oa.UserName || ga.Inode != oa.Inode || ga.Atime != oa.Atime ||
		!bytes.Equal(ga.Md5, oa.Md5) || len(ga.GroupName) != len(oa.GroupName) {
		t.Fatalf("attributes diverged:\n got=%+v\nwant=%+v", ga, oa)
	}
	for i := range oa.GroupName {
		if ga.GroupName[i] != oa.GroupName[i] {
			t.Fatalf("group[%d] = %q, want %q", i, ga.GroupName[i], oa.GroupName[i])
		}
	}

	// Chunks + nested FileId
	if len(got.Chunks) != 1 {
		t.Fatalf("chunks len = %d, want 1", len(got.Chunks))
	}
	gc, oc := got.Chunks[0], orig.Chunks[0]
	if gc.FileId != oc.FileId || gc.Size != oc.Size || gc.ModifiedTsNs != oc.ModifiedTsNs ||
		gc.ETag != oc.ETag || gc.IsCompressed != oc.IsCompressed || gc.SseType != oc.SseType ||
		!bytes.Equal(gc.CipherKey, oc.CipherKey) {
		t.Fatalf("chunk diverged:\n got=%+v\nwant=%+v", gc, oc)
	}
	if gc.Fid == nil || gc.Fid.VolumeId != oc.Fid.VolumeId || gc.Fid.FileKey != oc.Fid.FileKey || gc.Fid.Cookie != oc.Fid.Cookie {
		t.Fatalf("chunk Fid diverged: got=%+v want=%+v", gc.Fid, oc.Fid)
	}

	// Extended map
	if len(got.Extended) != len(orig.Extended) {
		t.Fatalf("extended len = %d, want %d", len(got.Extended), len(orig.Extended))
	}
	for k, v := range orig.Extended {
		if !bytes.Equal(got.Extended[k], v) {
			t.Fatalf("extended[%q] = %q, want %q", k, got.Extended[k], v)
		}
	}

	// RemoteEntry
	gr, or := got.RemoteEntry, orig.RemoteEntry
	if gr == nil || gr.StorageName != or.StorageName || gr.LastLocalSyncTsNs != or.LastLocalSyncTsNs ||
		gr.RemoteETag != or.RemoteETag || gr.RemoteMtime != or.RemoteMtime || gr.RemoteSize != or.RemoteSize {
		t.Fatalf("remote entry diverged:\n got=%+v\nwant=%+v", gr, or)
	}
}
