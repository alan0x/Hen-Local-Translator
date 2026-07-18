use crossbeam_channel::{bounded, Receiver, Sender};
use moxin_dora_bridge::{
    controller::DataflowController, dispatcher::DynamicNodeDispatcher, DoraStatus, SharedDoraState,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

enum RuntimeCommand {
    Start {
        dataflow_path: PathBuf,
        env_vars: HashMap<String, String>,
    },
    Stop,
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    Started(String),
    Stopped,
    Error(String),
}

pub struct TranslationRuntime {
    shared_state: Arc<SharedDoraState>,
    command_tx: Sender<RuntimeCommand>,
    event_rx: Receiver<RuntimeEvent>,
    stop_tx: Option<Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl TranslationRuntime {
    pub fn new() -> Self {
        let (command_tx, command_rx) = bounded(16);
        let (event_tx, event_rx) = bounded(32);
        let (stop_tx, stop_rx) = bounded(1);
        let running = Arc::new(AtomicBool::new(false));
        let shared_state = SharedDoraState::new();
        let worker_running = Arc::clone(&running);
        let worker_state = Arc::clone(&shared_state);

        let worker = thread::spawn(move || {
            let mut dispatcher: Option<DynamicNodeDispatcher> = None;
            let mut started_at: Option<Instant> = None;
            let mut last_status_check = Instant::now();

            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                while let Ok(command) = command_rx.try_recv() {
                    match command {
                        RuntimeCommand::Start {
                            dataflow_path,
                            env_vars,
                        } => {
                            if let Some(mut previous) = dispatcher.take() {
                                let _ = previous.stop();
                                thread::sleep(Duration::from_millis(350));
                            }

                            for (key, value) in &env_vars {
                                std::env::set_var(key, value);
                            }

                            let result = DataflowController::new(&dataflow_path).and_then(
                                |mut controller| {
                                    controller.set_envs(env_vars);
                                    let mut next = DynamicNodeDispatcher::with_shared_state(
                                        controller,
                                        Arc::clone(&worker_state),
                                    );
                                    match next.start() {
                                        Ok(id) => Ok((next, id)),
                                        Err(error) => {
                                            let _ = next.stop();
                                            Err(error)
                                        }
                                    }
                                },
                            );

                            match result {
                                Ok((next, id)) => {
                                    let active_bridges = next
                                        .bindings()
                                        .into_iter()
                                        .filter(|binding| {
                                            binding.state
                                                == moxin_dora_bridge::BridgeState::Connected
                                        })
                                        .map(|binding| binding.node_id.clone())
                                        .collect();
                                    worker_state.status.set(DoraStatus {
                                        active_bridges,
                                        last_error: None,
                                    });
                                    worker_running.store(true, Ordering::Release);
                                    worker_state.translation_overlay_active.set(true);
                                    worker_state
                                        .translation_overlay_status
                                        .set("warming".into());
                                    started_at = Some(Instant::now());
                                    dispatcher = Some(next);
                                    let _ = event_tx.send(RuntimeEvent::Started(id));
                                }
                                Err(error) => {
                                    let message =
                                        format!("Failed to start translation dataflow: {error}");
                                    worker_running.store(false, Ordering::Release);
                                    worker_state.status.set(DoraStatus {
                                        active_bridges: Vec::new(),
                                        last_error: Some(message.clone()),
                                    });
                                    worker_state.translation_overlay_active.set(false);
                                    worker_state.translation_overlay_status.set("idle".into());
                                    let _ = event_tx.send(RuntimeEvent::Error(message));
                                }
                            }
                        }
                        RuntimeCommand::Stop => {
                            if let Some(mut current) = dispatcher.take() {
                                let _ = current.stop();
                                thread::sleep(Duration::from_millis(300));
                            }
                            worker_running.store(false, Ordering::Release);
                            worker_state.status.set(DoraStatus::default());
                            worker_state.translation_overlay_active.set(false);
                            worker_state.translation_overlay_status.set("idle".into());
                            started_at = None;
                            let _ = event_tx.send(RuntimeEvent::Stopped);
                        }
                    }
                }

                let in_startup_grace = started_at
                    .map(|started| started.elapsed() < Duration::from_secs(10))
                    .unwrap_or(false);
                if !in_startup_grace && last_status_check.elapsed() >= Duration::from_secs(2) {
                    last_status_check = Instant::now();
                    if let Some(current) = &dispatcher {
                        if let Ok(status) = current.controller().read().get_status() {
                            let was_running = worker_running.load(Ordering::Acquire);
                            if was_running && !status.state.is_running() {
                                worker_running.store(false, Ordering::Release);
                                worker_state.translation_overlay_active.set(false);
                                worker_state.translation_overlay_status.set("idle".into());
                                started_at = None;
                                let _ = event_tx.send(RuntimeEvent::Stopped);
                            }
                        }
                    }
                }

                thread::sleep(Duration::from_millis(12));
            }

            if let Some(mut current) = dispatcher {
                let _ = current.stop();
            }
        });

        Self {
            shared_state,
            command_tx,
            event_rx,
            stop_tx: Some(stop_tx),
            worker: Some(worker),
        }
    }

    pub fn start(&self, dataflow_path: PathBuf) -> Result<(), String> {
        self.command_tx
            .try_send(RuntimeCommand::Start {
                dataflow_path,
                env_vars: HashMap::new(),
            })
            .map_err(|error| format!("Could not submit translation start command: {error}"))
    }

    pub fn stop(&self) -> Result<(), String> {
        self.command_tx
            .try_send(RuntimeCommand::Stop)
            .map_err(|error| format!("Could not submit translation stop command: {error}"))
    }

    pub fn shared_state(&self) -> &Arc<SharedDoraState> {
        &self.shared_state
    }

    pub fn poll_events(&self) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for TranslationRuntime {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
