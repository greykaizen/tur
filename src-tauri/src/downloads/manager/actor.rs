use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::commands::{DownloadRequest, ManagerCommand, NewDownloadItem};
use super::handle::ManagerHandle;
use super::state::ActiveDownload;
use super::utils::resolve_destination_conflict;
// Submodules
pub mod actions;
pub mod queue;
pub mod scheduler;

use crate::database::{Database, Queue};
use crate::downloads::client;
use crate::downloads::download::Download;
use crate::downloads::workers::run_download;
use crate::settings::{self, AppSettings};

pub struct ManagerActor {
    pub instances: HashMap<Uuid, ActiveDownload>,
    pub cmd_rx: mpsc::Receiver<ManagerCommand>,
    pub app: AppHandle,
    pub self_handle: ManagerHandle,
    pub boundary_timer_handle: Option<JoinHandle<()>>,
}

impl ManagerActor {
    fn get_db(&self) -> Result<Database, String> {
        Database::initialize(&self.app).map_err(|e| e.to_string())
    }

    pub async fn run(&mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                ManagerCommand::Start { request, reply_tx } => {
                    let settings = settings::load_or_create(&self.app);
                    match self.get_db() {
                        Ok(db) => {
                            let client_res = client::create(&settings);
                            match client_res {
                                Ok(client) => {
                                    match request {
                                        DownloadRequest::New(items) => {
                                            let res = match actions::add_new_downloads(
                                                &db, &client, &settings, items, &self.app,
                                            )
                                            .await
                                            {
                                                Ok(ids) => {
                                                    // Reconcile/Start
                                                    for id in ids {
                                                        if let Ok(Some(info)) =
                                                            db.get_download_by_id(&id)
                                                        {
                                                            if let Some(qid) = info.queue_id {
                                                                // Reconcile queue
                                                                let _ = queue::reconcile_queue(
                                                                    &mut self.instances,
                                                                    &db,
                                                                    &settings,
                                                                    &qid,
                                                                    &self.app,
                                                                    self.self_handle.clone(),
                                                                )
                                                                .await;
                                                            } else {
                                                                // Start immediately
                                                                let _ = actions::start_downloads(
                                                                    &mut self.instances,
                                                                    &db,
                                                                    &client,
                                                                    &settings,
                                                                    vec![id],
                                                                    &self.app,
                                                                    self.self_handle.clone(),
                                                                )
                                                                .await;
                                                            }
                                                        }
                                                    }
                                                    Ok(())
                                                }
                                                Err(e) => Err(e),
                                            };
                                            let _ = reply_tx.send(res);
                                        }
                                        DownloadRequest::Resume(uuids) => {
                                            let res = actions::start_downloads(
                                                &mut self.instances,
                                                &db,
                                                &client,
                                                &settings,
                                                uuids,
                                                &self.app,
                                                self.self_handle.clone(),
                                            )
                                            .await;
                                            let _ = reply_tx.send(res);
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = reply_tx.send(Err(e.to_string()));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = reply_tx.send(Err(e.to_string()));
                        }
                    }
                }
                ManagerCommand::Pause { id, reply_tx } => {
                    let _ =
                        reply_tx.send(actions::pause_download(&mut self.instances, id, &self.app));
                }
                ManagerCommand::Cancel { id, reply_tx } => {
                    let _ =
                        reply_tx.send(actions::cancel_download(&mut self.instances, id, &self.app));
                }
                ManagerCommand::Delete { id, reply_tx } => {
                    let _ =
                        reply_tx.send(actions::delete_download(&mut self.instances, id, &self.app));
                }
                ManagerCommand::IsActive { id, reply_tx } => {
                    let _ = reply_tx.send(self.instances.contains_key(&id));
                }
                ManagerCommand::ActiveCount { reply_tx } => {
                    let _ = reply_tx.send(self.instances.len());
                }
                ManagerCommand::DownloadCompleted { id } => {
                    if let Some(_instance) = self.instances.remove(&id) {
                        tracing::info!("⬇️ Download {} finished, removed from manager", id);
                    }
                    if let Err(e) = self.handle_download_completed_hook(id).await {
                        tracing::error!("Error in completion hook: {}", e);
                    }
                    self.handle_check_queue().await;
                }
                ManagerCommand::CheckQueue => {
                    self.handle_check_queue().await;
                }
                ManagerCommand::Shutdown { reply_tx } => {
                    self.shutdown_all();
                    let _ = reply_tx.send(());
                    break;
                }
                ManagerCommand::ReorderQueue {
                    queue_id,
                    new_order,
                    reply_tx,
                } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            let settings = settings::load_or_create(&self.app);
                            queue::handle_reorder_queue(
                                &mut self.instances,
                                &db,
                                &settings,
                                queue_id,
                                new_order,
                                &self.app,
                                self.self_handle.clone(),
                            )
                            .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::PauseQueue { queue_id, reply_tx } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            queue::handle_pause_queue(&mut self.instances, &db, queue_id, &self.app)
                                .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::ResumeQueue { queue_id, reply_tx } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            let settings = settings::load_or_create(&self.app);
                            queue::handle_resume_queue(
                                &mut self.instances,
                                &db,
                                &settings,
                                queue_id,
                                &self.app,
                                self.self_handle.clone(),
                            )
                            .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::CancelQueue { queue_id, reply_tx } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            queue::handle_cancel_queue(
                                &mut self.instances,
                                &db,
                                queue_id,
                                &self.app,
                            )
                            .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::AddToQueue {
                    download_id,
                    queue_id,
                    position,
                    reply_tx,
                } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            let settings = settings::load_or_create(&self.app);
                            queue::handle_add_to_queue(
                                &mut self.instances,
                                &db,
                                &settings,
                                download_id,
                                queue_id,
                                position,
                                &self.app,
                                self.self_handle.clone(),
                            )
                            .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::RemoveFromQueue {
                    download_id,
                    reply_tx,
                } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            let settings = settings::load_or_create(&self.app);
                            queue::handle_remove_from_queue(
                                &mut self.instances,
                                &db,
                                &settings,
                                download_id,
                                &self.app,
                                self.self_handle.clone(),
                            )
                            .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::SetSchedule {
                    download_id,
                    scheduled_at,
                    reply_tx,
                } => {
                    let db_res = self.get_db();
                    match db_res {
                        Ok(db) => {
                            let res = db
                                .set_download_schedule(&download_id, &scheduled_at)
                                .map_err(|e| e.to_string());
                            if res.is_ok() {
                                let settings = settings::load_or_create(&self.app);
                                scheduler::evaluate_schedules(
                                    &mut self.instances,
                                    &db,
                                    &settings,
                                    &self.app,
                                    self.self_handle.clone(),
                                    &mut self.boundary_timer_handle,
                                )
                                .await
                                .ok();
                            }
                            let _ = reply_tx.send(res);
                        }
                        Err(e) => {
                            let _ = reply_tx.send(Err(e));
                        }
                    }
                }
                ManagerCommand::ClearSchedule {
                    download_id,
                    reply_tx,
                } => match self.get_db() {
                    Ok(db) => {
                        let res = db
                            .clear_download_schedule(&download_id)
                            .map_err(|e| e.to_string());
                        if res.is_ok() {
                            let settings = settings::load_or_create(&self.app);
                            scheduler::evaluate_schedules(
                                &mut self.instances,
                                &db,
                                &settings,
                                &self.app,
                                self.self_handle.clone(),
                                &mut self.boundary_timer_handle,
                            )
                            .await
                            .ok();
                        }
                        let _ = reply_tx.send(res);
                    }
                    Err(e) => {
                        let _ = reply_tx.send(Err(e));
                    }
                },
                ManagerCommand::StartNow {
                    download_id,
                    reply_tx,
                } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            let settings = settings::load_or_create(&self.app);
                            scheduler::handle_start_now(
                                &mut self.instances,
                                &db,
                                &settings,
                                download_id,
                                &self.app,
                                self.self_handle.clone(),
                            )
                            .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::SetQueueDependency {
                    queue_id,
                    depends_on,
                    reply_tx,
                } => {
                    let res = match self.get_db() {
                        Ok(db) => {
                            queue::handle_set_queue_dependency(&db, queue_id, depends_on, &self.app)
                                .await
                        }
                        Err(e) => Err(e),
                    };
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::ClearQueueDependency { queue_id, reply_tx } => {
                    match self.get_db() {
                        Ok(db) => {
                            let res = db
                                .clear_queue_dependency(&queue_id)
                                .map_err(|e| e.to_string());
                            if res.is_ok() {
                                let settings = settings::load_or_create(&self.app);
                                queue::reconcile_queue(
                                    &mut self.instances,
                                    &db,
                                    &settings,
                                    &queue_id,
                                    &self.app,
                                    self.self_handle.clone(),
                                )
                                .await
                                .ok();
                            }
                            let _ = reply_tx.send(res);
                        }
                        Err(e) => {
                            let _ = reply_tx.send(Err(e));
                        }
                    }
                }
                ManagerCommand::EvaluateSchedules => {
                    if let Ok(db) = self.get_db() {
                        let settings = settings::load_or_create(&self.app);
                        if let Err(e) = scheduler::evaluate_schedules(
                            &mut self.instances,
                            &db,
                            &settings,
                            &self.app,
                            self.self_handle.clone(),
                            &mut self.boundary_timer_handle,
                        )
                        .await
                        {
                            tracing::error!("Error evaluating schedules: {}", e);
                        }
                    }
                }
                ManagerCommand::ApplyBandwidthLimit => {
                    let settings = settings::load_or_create(&self.app);
                    scheduler::apply_bandwidth_limit(&self.app, &settings).await;
                }
            }
        }
    }

    async fn handle_download_completed_hook(&mut self, _id: Uuid) -> Result<(), String> {
        let db = self.get_db()?;
        let settings = settings::load_or_create(&self.app);
        scheduler::evaluate_schedules(
            &mut self.instances,
            &db,
            &settings,
            &self.app,
            self.self_handle.clone(),
            &mut self.boundary_timer_handle,
        )
        .await
    }

    async fn handle_check_queue(&mut self) {
        let db = match self.get_db() {
            Ok(db) => db,
            Err(e) => {
                tracing::error!("Failed to init db for check queue: {}", e);
                return;
            }
        };
        let settings = settings::load_or_create(&self.app);

        match db.get_queues() {
            Ok(queues) => {
                for queue_data in queues {
                    if queue_data.status == "active" {
                        if let Err(e) = queue::reconcile_queue(
                            &mut self.instances,
                            &db,
                            &settings,
                            &queue_data.id,
                            &self.app,
                            self.self_handle.clone(),
                        )
                        .await
                        {
                            tracing::error!("Failed to reconcile queue {}: {}", queue_data.id, e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Queue check failed: {}", e);
            }
        }
    }

    fn shutdown_all(&mut self) {
        for instance in self.instances.values() {
            instance.stop();
        }
        self.instances.clear();
    }
}
