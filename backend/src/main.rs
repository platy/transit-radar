use std::error::Error;
use std::process;

mod gtfs;

fn example() -> Result<(), Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(std::fs::File::open("backend/gtfs/stops.txt")?);
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: gtfs::Stop = result?;
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
