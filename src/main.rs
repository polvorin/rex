use std::env;
mod types;
mod engine;
use types::RawTransaction;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <transactions.csv>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let engine = engine::Engine::new(); 

    let mut rdr = csv::Reader::from_path(path).expect("Failed to open CSV file");
    
    // This allocates memory for each transaction read, and further allocate and copy data around
    // between RawTransaction (used for reading CSV) with csv/serde, and the internal representation
    // used by the engine.  But it simplifies the code, and allow the engine to work over cleaner types.
    for row in rdr.deserialize() {
        let raw: RawTransaction = row.expect("Failed to deserialize CSV record");
        let tx = raw.into_transaction().expect("Failed to parse transaction type");
        engine.process_transaction(tx);
    }
}