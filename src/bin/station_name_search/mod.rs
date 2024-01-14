use lazysort::SortedBy;
use std::cmp::Ordering;
use urlencoding::decode;

use radar_search::search_data::*;
use transit_radar::Suggester;

fn most_important((id1, imp1): &(StopId, usize), (id2, imp2): &(StopId, usize)) -> Ordering {
    imp1.cmp(imp2).reverse().then(id1.cmp(id2))
}

pub fn station_search_handler<'d>(
    query: &str,
    data: &'d GTFSData,
    station_search: &Suggester<(ZoneInternKey, usize)>,
) -> Result<impl IntoIterator<Item = Zone<'d>>, ()> {
    const RESULT_LIMIT: usize = 20;
    match decode(query) {
        Ok(query) => {
            let matches = station_search.search(&query);
            let top_matches = matches
                .into_iter()
                .sorted_by(most_important)
                .take(RESULT_LIMIT)
                .map(move |(zone_key, _importance)| {
                    data.get_zone_by_key(zone_key)
                });
            Ok(top_matches)
        }
        Err(err) => {
            eprintln!("dir: failed to decode query={:?}: {:?}", query, err);
            Err(())
        }
    }
}
