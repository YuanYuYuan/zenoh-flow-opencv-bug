# Zenoh-flow OpenCV Bug Report

This repo just serves for this [issue](https://github.com/eclipse-zenoh/zenoh-flow/issues/127).
That is if we build the OpenCV with QT and run the example, Qt would complain it can't be started from another thread.
(This example is adapted from simple video-pipeline in zenoh-flow-examples 0.3.0, but the problem also exists in zenoh-flow dev/0.4.0.)

## How to reproduce

1. Build OpenCV with Qt as GUI backend

```bash
./install-opencv-4.6.sh
```

2. Specify the built OpenCV libaray

```bash
source ./setup-opencv-lib.sh
```

3. Prepare a sample video and modify the position in the `src/main.rs`

```rust
static SOURCE: &str = "Frame";
static INPUT: &str = "Frame";
static FILE: &str = "FILL_IN_A_SAMPLE_VIDEO";
```

4. Run the program

```bash
cargo run
```

## Bug

```bash
QObject::startTimer: Timers cannot be started from another thread
```
