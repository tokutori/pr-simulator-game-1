# openxr

Source: the published `openxr 0.21.1` crate, MIT OR Apache-2.0.
Archive: https://static.crates.io/crates/openxr/openxr-0.21.1.crate
SHA-256: `2c2f814a47d257c79c762bc64042859661b6aa2f57cfa1f0ad4d778f419aeb0e`.
The crate's own lockfile is omitted; the application workspace lockfile resolves
the dependency. Published sources and license files are retained.
Trailing whitespace in the README heading is normalized.

`Swapchain::wait_image` returns the original success status. `XR_TIMEOUT_EXPIRED`
leaves the acquired image unwaited, so callers can wait on that same image again.
`XR_SESSION_LOSS_PENDING` remains visible to the caller. The existing FFI call and
its unsafe boundary are unchanged.

The used primary frame wait/begin/end, acquire/release and exit-request calls also
return their original success statuses. The consumer distinguishes session loss
from normal completion while draining outstanding frames and GPU submissions.

The timeout is an OpenXR wait parameter, not a hard wall-clock deadline. The
application retains I/O custody when an operation has not reached its terminal
result.
