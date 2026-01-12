use tokio::sync::oneshot;

/// Request from worker to coordinator
#[derive(Debug)]
pub struct WorkRequest {
    pub worker_id: usize,
    pub reply_tx: oneshot::Sender<WorkResponse>,
}

/// Response from coordinator to worker
#[derive(Debug)]
pub enum WorkResponse {
    /// New range assigned: (start_unit, end_unit)
    Range(usize, usize),
    /// Stolen range: (start_unit, end_unit, victim_id)
    Stolen(usize, usize, usize),
    /// No work available
    None,
}
