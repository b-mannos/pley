# pley-transcoder

The FFmpeg media engine of the [pley](../../README.md) media server. It is a
standalone HTTP service that handles:

- probing media files
- HLS transcoding and remuxing
- HDR10, HLG and Dolby Vision tone mapping
- subtitle burn-in and extraction
- thumbnails and cover art
- metrics in JSON and Prometheus formats

It is a separate program. The pley server talks to
it only over HTTP; see `../../LICENSING.md`.

```sh
cargo run -p pley-transcoder -- --listen 127.0.0.1:7540 --token secret
curl -H "Authorization: Bearer secret" http://127.0.0.1:7540/v1/info
```

The protocol, the HLS job model and the feature list are documented in
[`docs/TRANSCODER.md`](../../docs/TRANSCODER.md).
