# bevy_mod_openxr

Source: the published `bevy_mod_openxr 0.6.0` crate, MIT OR Apache-2.0.
Archive: https://static.crates.io/crates/bevy_mod_openxr/bevy_mod_openxr-0.6.0.crate
SHA-256: `af0560ba6db564db1e3b60dfc9abf359443a33ba9295acfd18aff8996b8f6288`.

The archive's `.cargo_vcs_info.json` is retained. Its revision is not currently
reachable in the advertised GitHub repository. License texts are from the same
official repository at `18b64bec26f343931f1b439d2e4a4013f6a095e8`.
The crate's own lockfile is omitted; the application workspace lockfile resolves
the dependency. Published library sources and examples are retained.

Local changes:

- Swapchain texture wrappers use shared owning `Arc<[wgpu::Texture]>` storage;
  extraction clones that ownership instead of leaking a static slice.
- Session creation returns its original error and emits the created schedule
  and message after successful acquisition.
- The existing pending render-resource carrier is public so a caller can move
  the acquired frame stream, swapchain and image ownership into its own I/O
  thread instead of registering the default rendering systems.
- Normal wgpu and XR utility dependencies disable their default features.
- The used frame, image and exit APIs preserve the status from the patched binding.
- Projection layers borrow an owned OpenXR reference space directly, bypassing
  the optional space-patching plugin. Stereo targets allow transfer destinations.

Birdman supplies the camera, frame, input and release integration. The upstream
automatic camera and rendering plugins are not registered by that integration.
