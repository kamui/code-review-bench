// Probe P1, part 4. Reproduces the event text reported in kubernetes/kubernetes#112621 (2022): an error built with
// fmt.Errorf("...: %w", nil) and then used as a format string with no arguments, as the scheduler's Eventf call does.
package main

import (
	"fmt"
)

func main() {
	var cause error
	errMsg := fmt.Errorf("running PreFilter plugin %q: %w", "VolumeBinding", cause).Error()
	fmt.Printf("error text:                 %q\n", errMsg)
	fmt.Printf("used as a format, no args:  %q\n", fmt.Sprintf(errMsg))
	fmt.Printf("used with \"%%s\":             %q\n", fmt.Sprintf("%s", errMsg))
}
