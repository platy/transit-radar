use lazysort::SortedBy;
use serde::Serialize;
use std::cmp::Ordering;
use std::sync::Arc;
use urlencoding::decode;
use warp::Filter;

use radar_search::search_data::*;
use transit_radar::Suggester;

use super::with_data;

#[derive(Serialize)]
struct FEStationLookup<'s> {
    stop_id: StopId,
    name: &'s str,
}

fn most_important((id1, imp1): &(StopId, usize), (id2, imp2): &(StopId, usize)) -> Ordering {
    imp1.cmp(imp2).reverse().then(id1.cmp(id2))
}

async fn station_search_handler(
    query: String,
    data: Arc<GTFSData>,
    station_search: Arc<Suggester<(StopId, usize)>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    const RESULT_LIMIT: usize = 20;
    match decode(&query) {
        Ok(query) => {
            let matches = station_search.search(&query);
            let top_matches = matches
                .into_iter()
                .sorted_by(most_important)
                .take(RESULT_LIMIT);
            let result: Vec<FEStationLookup> = top_matches
                .map(|(stop_id, _importance)| {
                    let stop = data
                        .get_stop(stop_id)
                        .expect("to find stop referenced by search");
                    FEStationLookup {
                        stop_id,
                        name: &stop.stop_name,
                    }
                })
                .collect();
            Ok(warp::reply::json(&result))
        }
        Err(err) => {
            eprintln!("dir: failed to decode query={:?}: {:?}", &query, err);
            Err(warp::reject::not_found())
        }
    }
}

pub fn station_name_search_route(
    data: Arc<GTFSData>,
    station_search: Arc<Suggester<(StopId, usize)>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors().allow_any_origin();
    warp::path!("searchStation" / String)
        .and(with_data(data))
        .and(with_data(station_search))
        .and_then(station_search_handler)
        .with(cors)
}
