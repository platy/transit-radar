use std::error::Error;
use std::collections::HashSet;
use std::path::Path;
use transit_radar::gtfs::db::GTFSSource;
use transit_radar::gtfs::{StopTime, Route, Trip, RouteType};

fn print_record(record: &csv::StringRecord) {
  for field in record.into_iter() {
      print!("{},", field);
  }
  println!();
}

fn mainr() -> Result<(), Box<dyn Error>> {
  let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or("gtfs".to_owned());
  let gtfs_dir = Path::new(&gtfs_dir);
  let source = &GTFSSource::new(gtfs_dir);

  let mut route_ids = HashSet::new();
  let mut rdr = source.open_csv("routes.txt")?;
  for result in rdr.deserialize() {
      let route: Route = result?;
      if [RouteType::UrbanRailway, RouteType::SuburbanRailway].contains(&route.route_type) { // ubahn / sbahn
          route_ids.insert(route.route_id);
      }
  }

  let mut trip_ids = HashSet::new();
  let mut rdr = source.open_csv("trips.txt")?;
  for result in rdr.deserialize() {
      let trip: Trip = result?;
      if route_ids.contains(&trip.route_id) {
          trip_ids.insert(trip.trip_id);
      }
  }

  eprintln!("Emitting {} routes {} trips", route_ids.len(), trip_ids.len());

  let path = gtfs_dir.join("stop_times.txt");
  eprintln!("Opening {}", path.to_str().expect("path invalid"));
  let mut rdr = csv::Reader::from_path(path)?;
  let headers = rdr.headers().expect("to be able to read headers");
  print_record(headers);

  while let Some(record) = rdr.records().next() {
      let record = record?;
      let stop_time: StopTime = record.deserialize(Some(rdr.headers()?))?;
      if trip_ids.contains(&stop_time.trip_id) {
          print_record(&record);
      }
  }
  Ok(())
}

fn main() {
  mainr().unwrap()
}
