use std::sync::Arc;
use warp::Filter;

pub fn with_data<D: Sync + Send>(
    db: Arc<D>,
) -> impl Filter<Extract = (Arc<D>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}
