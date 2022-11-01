//
// Copyright (c) 2022 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

use async_ctrlc::CtrlC;
use async_trait::async_trait;
use opencv::{core, highgui, prelude::*, videoio};
use zenoh_flow::async_std::stream::StreamExt;
use zenoh_flow::async_std::sync::{Arc, Mutex};
use zenoh_flow::model::{InputDescriptor, OutputDescriptor};
use zenoh_flow::runtime::dataflow::instance::DataflowInstance;
use zenoh_flow::runtime::dataflow::loader::{Loader, LoaderConfig};
use zenoh_flow::runtime::RuntimeContext;
use zenoh_flow::Configuration;
use zenoh_flow::{
    model::link::PortDescriptor, zenoh_flow_derive::ZFState, zf_spin_lock, Data, Node, Sink,
    Source, State, ZFResult,
};

static SOURCE: &str = "Frame";
static INPUT: &str = "Frame";

#[derive(Debug)]
struct CameraSource;

#[derive(ZFState, Clone)]
struct CameraState {
    pub camera: Arc<Mutex<videoio::VideoCapture>>,
    pub encode_options: Arc<Mutex<opencv::types::VectorOfi32>>,
    pub resolution: (i32, i32),
    pub delay: u64,
}

// Because of opencv
impl std::fmt::Debug for CameraState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "CameraState: resolution:{:?} delay:{:?}",
            self.resolution, self.delay
        )
    }
}

impl CameraState {
    fn new() -> Self {
        let camera = videoio::VideoCapture::new(0, videoio::CAP_ANY).unwrap(); // 0 is the default camera
        // let camera = videoio::VideoCapture::from_file(FILE, videoio::CAP_ANY).unwrap();
        let opened = videoio::VideoCapture::is_opened(&camera).unwrap();
        if !opened {
            panic!("Unable to open default camera!");
        }
        let mut encode_options = opencv::types::VectorOfi32::new();
        encode_options.push(opencv::imgcodecs::IMWRITE_JPEG_QUALITY);
        encode_options.push(90);

        Self {
            camera: Arc::new(Mutex::new(camera)),
            encode_options: Arc::new(Mutex::new(encode_options)),
            resolution: (800, 600),
            // resolution: (1280, 720),
            delay: 40,
        }
    }
}

#[async_trait]
impl Source for CameraSource {
    async fn run(
        &self,
        _context: &mut zenoh_flow::Context,
        dyn_state: &mut State,
    ) -> zenoh_flow::ZFResult<Data> {
        // Downcasting to right type
        let state = dyn_state.try_get::<CameraState>()?;
        let data: Vec<u8>;

        {
            let mut cam = zf_spin_lock!(state.camera);
            let encode_options = zf_spin_lock!(state.encode_options);

            let mut frame = core::Mat::default();
            cam.read(&mut frame).unwrap();

            let mut reduced = Mat::default();
            opencv::imgproc::resize(
                &frame,
                &mut reduced,
                opencv::core::Size::new(state.resolution.0, state.resolution.1),
                0.0,
                0.0,
                opencv::imgproc::INTER_LINEAR,
            )
            .unwrap();

            let mut buf = opencv::types::VectorOfu8::new();
            opencv::imgcodecs::imencode(".jpeg", &reduced, &mut buf, &encode_options).unwrap();

            data = buf.into();

            drop(cam);
            drop(encode_options);
        }

        async_std::task::sleep(std::time::Duration::from_millis(state.delay)).await;
        Ok(Data::from_bytes(data))
    }
}

impl Node for CameraSource {
    fn initialize(&self, _configuration: &Option<Configuration>) -> ZFResult<State> {
        Ok(State::from(CameraState::new()))
    }

    fn finalize(&self, _state: &mut State) -> ZFResult<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct SimpleSink {
    tx: flume::Sender<Vec<u8>>,
}

#[derive(ZFState, Clone, Debug)]
struct DummyState;

impl Node for SimpleSink {
    fn initialize(&self, _configuration: &Option<Configuration>) -> ZFResult<State> {
        Ok(State::from(DummyState))
    }

    fn finalize(&self, _state: &mut State) -> ZFResult<()> {
        Ok(())
    }
}

#[async_trait]
impl Sink for SimpleSink {
    async fn run(
        &self,
        _context: &mut zenoh_flow::Context,
        _dyn_state: &mut State,
        mut input: zenoh_flow::runtime::message::DataMessage,
    ) -> zenoh_flow::ZFResult<()> {
        let data = input.get_inner_data().try_as_bytes()?.as_ref().clone();
        self.tx.send(data)?;
        Ok(())
    }
}

#[async_std::main]
async fn main() {
    env_logger::init();

    let session = Arc::new(zenoh::open(zenoh::config::Config::default()).await.unwrap());
    let hlc = async_std::sync::Arc::new(uhlc::HLC::default());

    let ctx = RuntimeContext {
        session,
        hlc,
        loader: Arc::new(Loader::new(LoaderConfig::new())),
        runtime_name: String::from("local").into(),
        runtime_uuid: uuid::Uuid::new_v4(),
    };

    let mut zf_graph =
        zenoh_flow::runtime::dataflow::Dataflow::new(ctx.clone(), "video-pipeline".into(), None);

    let (tx, rx) = flume::bounded(2);
    let source = Arc::new(CameraSource);
    let sink = Arc::new(SimpleSink { tx });

    zf_graph
        .try_add_static_source(
            "camera-source".into(),
            None,
            PortDescriptor {
                port_id: String::from(SOURCE).into(),
                port_type: String::from("image").into(),
            },
            source.initialize(&None).unwrap(),
            source,
        )
        .unwrap();

    zf_graph
        .try_add_static_sink(
            "video-sink".into(),
            PortDescriptor {
                port_id: String::from(INPUT).into(),
                port_type: String::from("image").into(),
            },
            sink.initialize(&None).unwrap(),
            sink,
        )
        .unwrap();

    zf_graph
        .try_add_link(
            OutputDescriptor {
                node: "camera-source".into(),
                output: String::from(SOURCE).into(),
            },
            InputDescriptor {
                node: "video-sink".into(),
                input: String::from(INPUT).into(),
            },
            None,
            None,
            None,
        )
        .unwrap();

    let mut instance = DataflowInstance::try_instantiate(zf_graph).unwrap();

    let nodes = instance.get_nodes();
    for id in &nodes {
        instance.start_node(id).await.unwrap()
    }

    async_std::task::spawn_blocking(move || {
        loop {
            while let Ok(data) = rx.recv() {
                let decoded = opencv::imgcodecs::imdecode(
                    &opencv::types::VectorOfu8::from_iter(data),
                    opencv::imgcodecs::IMREAD_COLOR,
                )
                .unwrap();
                if decoded.size().unwrap().width > 0 {
                    highgui::imshow("Test", &decoded).unwrap();
                }
                highgui::wait_key(10).unwrap();
            }
        }
    });

    let ctrlc = CtrlC::new().expect("Unable to create Ctrl-C handler");
    let mut stream = ctrlc.enumerate().take(1);
    stream.next().await;
    println!("Received Ctrl-C start teardown");

    for id in nodes {
        instance.stop_node(&id).await.unwrap()
    }
}
