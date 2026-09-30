# Rubric-v2 site release

The site must use the new rubric-v2 grades and scores exclusively. Preserve earlier grades, results, frozen reviewer inputs and raw reviews for archival use. Remove historical scoring controls, old scoreboard links and fallback paths from the site and its exporter.

Pin each result's mapping versions and the current reference versions. Keep reviewer manifests unchanged. Retain task comparability checks; regrading does not make different reference versions comparable. New, unresolved claims remain unresolved until their saved ruling exists.

Include the three approved fatal-error recovery attempts in their original trials. Preserve their failed predecessors and include all recorded usage. A recovery earns detection credit only when its review is valid. Incomplete usage remains unavailable. The separate derived normalization recovery for rclone does not change that attempt's original disposition.

Record blinding qualifications in downloadable data and audit records. Do not display a blinding notice on the site. Wider reference audits remain pending; this release does not claim human-audited ground truth or a fully blinded comparison.

The explorer build contains 607 attempts in 18 configurations, 12 PR tasks, 17 reference problems, five review methods and eight models. The three added problems retain proposed concern labels and unadjudicated severity. No new paid model calls are needed.

Publish the completed, verified change as a PR against main. Deployment follows merge.
