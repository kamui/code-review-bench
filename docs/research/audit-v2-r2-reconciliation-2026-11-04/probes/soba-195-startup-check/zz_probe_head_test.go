package internal

import "strings"

func probeDirectThreeParam() (string, int) {
	var b strings.Builder
	return "checkUserAndPasswordProvider", checkUserAndPasswordProvider(providerNameBitBucketOAuth, &b)
}
