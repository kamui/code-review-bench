package internal

func probeDirectThreeParam() (string, int) {
	n, _ := checkProvider(providerNameBitBucketOAuth)
	return "checkProvider", n
}
