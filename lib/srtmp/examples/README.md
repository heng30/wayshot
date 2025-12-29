## How to test?

- run the [xiu](https://github.com/harlanc/xiu) media server: `./xiu-rtmp-server.sh`

- run the `rtmp client demo`: `RUST_LOG=debug cargo run --example rtmp-client-demo`

- run the `ffplay` to play the video: `./ffplay-pull-client.sh`

