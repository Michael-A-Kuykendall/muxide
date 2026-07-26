// Command example demonstrates basic usage of the muxide Go bindings.
package main

import (
	"fmt"
	"os"

	muxide "github.com/Michael-A-Kuykendall/muxide/bindings/go"
)

func main() {
	m, err := muxide.NewMuxer(muxide.CodecH264, 1280, 720, 30.0)
	if err != nil {
		fmt.Fprintln(os.Stderr, "new muxer:", err)
		os.Exit(1)
	}
	defer func() {
		if err := m.Close(); err != nil {
			fmt.Fprintln(os.Stderr, "close:", err)
		}
	}()

	keyframe := []byte{0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f}
	if err := m.WriteVideo(0.0, keyframe, true); err != nil {
		fmt.Fprintln(os.Stderr, "write video:", err)
		os.Exit(1)
	}
	if err := m.WriteVideo(1.0/30.0, []byte{0x00, 0x00, 0x00, 0x01, 0x41}, false); err != nil {
		fmt.Fprintln(os.Stderr, "write video:", err)
		os.Exit(1)
	}

	out, err := m.Finish()
	if err != nil {
		fmt.Fprintln(os.Stderr, "finish:", err)
		os.Exit(1)
	}
	fmt.Printf("wrote %d bytes of MP4\n", len(out))
}
