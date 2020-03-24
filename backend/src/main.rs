use std::error::Error;
use std::process;
use std::collections::HashMap;
use std::path::Path;
use db::GTFSSource;

mod gtfs;
use gtfs::*;
use gtfs::gtfstime::{Time, Period};

mod journey_graph;

fn example2(source: &GTFSSource) -> Result<(), Box<dyn Error>> {
    // let stops = self.stops_by_id(self.get_stops()?);
    let sunday_services = source.get_sunday_services()?;
    println!("{} services", sunday_services.len());
    let available_trips = source.get_trips(None, sunday_services, None)?;
    let available_trips: HashMap<TripId, Trip> = available_trips.into_iter().map(|trip| (trip.trip_id, trip)).collect();

    let departure_stops = source.stops_of_station(900000007103)?;
    println!("Departure stops : {:?}", departure_stops);
    let trips = source.non_branching_travel_times_from(&departure_stops, &available_trips, Time::parse("09:00:00")?)?;
    let stops_by_id = source.stops_by_id(source.get_stops()?);
    let routes_by_id = source.routes_by_id()?;
    for (trip_id, stops) in trips.iter() {
        println!("Route {} Trip {}", routes_by_id[&available_trips[trip_id].route_id].route_short_name, trip_id);
        for (stop_id, duration) in stops.iter() {
            println!("  {:>2}m {}", duration.mins(), stops_by_id[stop_id].stop_name);
        }
    }
    println!("{} trips shown", trips.len());
    Ok(())
}

use journey_graph::{QueueItemVariant};
use geo::algorithm::bearing::Bearing;

fn example3(source: &GTFSSource) -> Result<(), Box<dyn Error>> {
    let period = Period::between(Time::parse("19:00:00")?, Time::parse("19:30:00")?);

    let mut data;
    if let Some(data2) = source.load_cache(period)? {
        data = data2
    } else {
        data = gtfs::db::GTFSData::new();
        data.load_transfers_of_stop(source)?;
        data.load_stops_by_id(source)?;
        data.departure_lookup(period, &source)?;
        source.write_cache(period, &data)?;
    };

    let fake_stop: Stop = Stop::fake();

    let mut plotter = journey_graph::JourneyGraphPlotter::new(period, &data)?;
    let origin = data.get_stop(&900000007103).unwrap();
    for stop_id in data.stops_by_parent_id().get(&origin.stop_id).unwrap() {
        let stop = data.get_stop(stop_id).unwrap();
        plotter.add_origin(&fake_stop, stop);
    }
    // for item in plotter.filter_map(|(item, fastest)| if fastest { Some(item) } else { None }) {
    //     match item.variant {
    //         QueueItemVariant::StopOnTrip { trip } => {
    //         println!("{} Arrived at {:4.0} {} with {}", item.arrival_time, origin.position().bearing(item.to_stop.position()), &item.to_stop.stop_name, trip);
    //         },
    //         QueueItemVariant::Connection => {
    //             if item.to_stop.parent_station != item.from_stop.parent_station {
    //                 println!("{} Transferred to {}", item.arrival_time, &item.to_stop.stop_name);
    //             }
    //         }
    //     }
    // }

    let fe_arrivals: Vec<FEStop> = plotter.filter_map(
        |(item, fastest)| 
            if fastest { 
                Some(FEStop {
                    bearing: origin.position().bearing(item.to_stop.position()),
                    name: &item.to_stop.stop_name,
                    seconds: item.arrival_time - period.start(),
                }) 
            } else { 
                None 
            }).collect();
    serde_json::to_writer_pretty(std::io::stdout(), &fe_arrivals)?;
    println!();
    
    Ok(())
}

use serde::Serialize;

#[derive(Serialize)]
struct FEStop<'s> {
    bearing: f64,
    name: &'s str,
    seconds: gtfstime::Duration,
}

fn main() {
    if let Err(err) = example3(&GTFSSource::new(Path::new("./gtfs/"))) {
        println!("error running example: {:?}", err);
        process::exit(1);
    }
}
