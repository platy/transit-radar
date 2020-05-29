use std::sync::Arc;
use warp::Filter;

mod station_name_search_endpoint;

pub use station_name_search_endpoint::station_name_search_route;

pub fn with_data<D: Sync + Send>(
    db: Arc<D>,
) -> impl Filter<Extract = (Arc<D>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}
