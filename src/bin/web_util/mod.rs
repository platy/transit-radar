use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use warp::{reject, Filter};

pub mod naive_state;

pub fn with_data<D: Sync + Send>(
    db: Arc<D>,
) -> impl Filter<Extract = (Arc<D>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}
