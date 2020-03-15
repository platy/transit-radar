use std::error::Error;
use std::process;
use time::Time;

use serde::Deserialize;

type TripId = u64;
type StopId = String;

#[derive(Debug, Deserialize)]
struct StopTime { // "trip_id","arrival_time","departure_time","stop_id","stop_sequence","pickup_type","drop_off_type","stop_headsign"
    trip_id: TripId,
    #[serde(with = "de_time")]
    arrival_time: Time,
    #[serde(with = "de_time")]
    departure_time: Time,
    stop_id: StopId,
    stop_sequence: u32,
    pickup_type: u16,
    drop_off_type: u16,
    stop_headsign: Option<String>,
}

mod de_time {
    use time::Time;
    
    use serde::Deserialize;
    use serde::Deserializer;

    const FORMAT: &'static str = "%T";

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Time, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Time::parse(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}

fn example() -> Result<(), Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(std::fs::File::open("backend/gtfs/stop_times.txt")?);
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: StopTime = result?;
        println!("{:?}", record);
    }
    Ok(())
}

fn main() {
    if let Err(err) = example() {
        println!("error running example: {:?}", err);
        process::exit(1);
    }
}