use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

use transit_radar::gtfs::db;

fn main() {
    let gtfs_dir = std::env::var("GTFS_DIR").unwrap_or("gtfs".to_owned());
    let gtfs_dir = Path::new(&gtfs_dir);

    let now = Instant::now();
    let data = db::load_data(&gtfs_dir, db::DayFilter::All).unwrap();
    println!("Reading from GTFS took {}s", now.elapsed().as_secs());

    // serde_json::to_writer_pretty(std::io::stdout(), &data);
    // serde_json::to_writer(std::io::stdout(), &data);
    // bincode::serialize_into(std::io::stdout(), &data).expect("Successful serialization");
    // serde_cbor::to_writer(std::io::stdout(), &data).expect("Successful serialization");
    let now = Instant::now();
    data.serialize(
        &mut rmp_serde::Serializer::new(
            std::fs::File::create("./search-data.messagepack").expect("file can be created"),
        )
        .with_struct_tuple()
        .with_integer_variants(),
    )
    .expect("Successful serialization");
    println!("Writing message pack took {}s", now.elapsed().as_secs());

    let now = Instant::now();
    let data: radar_search::search_data::GTFSData = rmp_serde::from_read(
        std::fs::File::open("./search-data.messagepack").expect("file can be opened"),
    )
    .expect("Succesful deserialization");
    println!("Reading message pack took {}s", now.elapsed().as_secs());
}
