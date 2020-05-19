use chrono::prelude::*;
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use urlencoding::decode;
use warp::Filter;

use radar_search::{journey_graph, search_data::*, search_data_sync::*, time::*};
use transit_radar::gtfs::db;
use transit_radar::Suggester;

use crate::web_util::*;

#[derive(Serialize)]
struct FEStationLookup<'s> {
    stop_id: u64,
    name: &'s str,
}

async fn station_search_handler(
    query: String,
    data: Arc<GTFSData>,
    station_search: Arc<Suggester<StopId>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match decode(&query) {
        Ok(query) => {
            let mut result = Vec::new();
            let mut count = 0;
            for stop_id in station_search.search(&query) {
                if count > 20 {
                    break;
                }
                let stop = data
                    .get_stop(&stop_id)
                    .expect("to find stop referenced by search");
                result.push(FEStationLookup {
                    stop_id,
                    name: &stop.stop_name,
                });
                count += 1;
            }
            Ok(warp::reply::json(&result))
        }
        Err(err) => {
            eprintln!("dir: failed to decode query={:?}: {:?}", &query, err);
            return Err(warp::reject::not_found());
        }
    }
}

pub fn station_name_search_route(
    data: Arc<GTFSData>,
    station_search: Arc<Suggester<StopId>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors().allow_any_origin();
    warp::path!("searchStation" / String)
        .and(with_data(data))
        .and(with_data(station_search))
        .and_then(station_search_handler)
        .with(cors)
}
