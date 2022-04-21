use lazysort::SortedBy;
use serde::Serialize;
use std::cmp::Ordering;
use urlencoding::decode;

use radar_search::search_data::*;
use transit_radar::Suggester;

#[derive(Serialize)]
struct FEStationLookup<'s> {
    stop_id: StopId,
    name: &'s str,
}

fn most_important((id1, imp1): &(StopId, usize), (id2, imp2): &(StopId, usize)) -> Ordering {
    imp1.cmp(imp2).reverse().then(id1.cmp(id2))
}

pub fn station_search_handler<'d>(
    query: &str,
    data: &'d GTFSData,
    station_search: &Suggester<(StopId, usize)>,
) -> Result<impl IntoIterator<Item = &'d Stop>, ()> {
    const RESULT_LIMIT: usize = 20;
    match decode(query) {
        Ok(query) => {
            let matches = station_search.search(&query);
            let top_matches = matches
                .into_iter()
                .sorted_by(most_important)
                .take(RESULT_LIMIT)
                .map(move |(stop_id, _importance)| {
                    data.get_stop(stop_id)
                        .expect("to find stop referenced by search")
                });
            Ok(top_matches)
        }
        Err(err) => {
            eprintln!("dir: failed to decode query={:?}: {:?}", query, err);
            Err(())
        }
    }
}
