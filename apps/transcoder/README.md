# pley-transcoder

The FFmpeg media engine of the [Pley](../../README.md) media server. It is a
standalone HTTP service that handles:

- probing media files
- HLS transcoding and remuxing
- HDR10, HLG and Dolby Vision tone mapping
- subtitle burn-in and extraction
- thumbnails and cover art
- metrics in JSON and Prometheus formats

It is Restricted, as all of Pley is (`../../LICENSE`), and the Pley server
talks to it only over HTTP; see `../../LICENSING.md`.

```sh
export PLEY_TRANSCODER_TOKEN=$(openssl rand -hex 16)
cargo run -p pley-transcoder -- --listen 127.0.0.1:7540
curl -H "Authorization: Bearer $PLEY_TRANSCODER_TOKEN" http://127.0.0.1:7540/v1/info
```

The token goes in the environment rather than `--token`: every account of
the computer can read a command line in its list of processes.

The protocol, the HLS job model and the feature list are documented in
[`docs/TRANSCODER.md`](../../docs/TRANSCODER.md).
