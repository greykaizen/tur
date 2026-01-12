/// Unit completion signal for triggering meta saves
#[derive(Debug, Clone)]
pub struct UnitCompletion {
    pub worker_id: usize,
    pub unit_index: usize,
}
