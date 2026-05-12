use std::env;
mod engine;
mod types;
use types::RawTransaction;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <transactions.csv>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let mut engine = engine::Engine::new();

    let mut rdr = csv::Reader::from_path(path).expect("Failed to open CSV file");

    // This allocates memory for each transaction read, and further allocate and copy data around
    // between RawTransaction (used for reading CSV) with csv/serde, and the internal representation
    // used by the engine.  But it simplifies the code, and allow the engine to work over cleaner types.
    for row in rdr.deserialize() {
        let raw: RawTransaction = row.expect("Failed to deserialize CSV record");
        let tx = raw
            .into_transaction()
            .expect("Failed to parse transaction type");
        engine.process_transaction(tx);
    }

    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(std::io::stdout());

    for record in engine.report().iter() {
        writer
            .serialize(record)
            .expect("Failed to write CSV record");
    }
}
