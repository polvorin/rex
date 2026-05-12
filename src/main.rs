use std::env;
use std::sync::mpsc::{self, SyncSender};
use std::thread;

mod engine;
mod types;
use types::{RawTransaction, Transaction};

// How many engines to run in parallel.
// Ideally this should be equal to the number of CPU cores,
// but to keep things simple and avoid another crate dependency,
// we hardcode it here
const ENGINE_COUNT: usize = 4;

// Channel between reading thread and processing engines is bounded
// to provide for backpressure.
const CHANNEL_CAPACITY: usize = 1024;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <transactions.csv>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let mut senders = Vec::with_capacity(ENGINE_COUNT);
    let mut handles = Vec::with_capacity(ENGINE_COUNT);

    for _ in 0..ENGINE_COUNT {
        let (tx, rx) = mpsc::sync_channel(CHANNEL_CAPACITY);
        let handle = thread::spawn(move || {
            let mut engine = engine::Engine::new();
            for transaction in rx {
                engine.process_transaction(transaction);
            }
            engine.report()
        });
        senders.push(tx);
        handles.push(handle);
    }

    let mut rdr = csv::Reader::from_path(path).expect("Failed to open CSV file");

    // This allocates memory for each transaction read, and further allocate and copy data around
    // between RawTransaction (used for reading CSV) with csv/serde, and the internal representation
    // used by the engine.  But it simplifies the code, and allow the engine to work over cleaner types.
    for row in rdr.deserialize() {
        let raw: RawTransaction = row.expect("Failed to deserialize CSV record");
        let tx = raw
            .into_transaction()
            .expect("Failed to parse transaction type");
        route_transaction(tx, &senders).expect("Failed to route transaction");
    }

    drop(senders);

    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(std::io::stdout());

    for handle in handles {
        let engine_report = handle
            .join()
            .expect("Engine thread panicked while generating report");

        for record in engine_report.iter() {
            writer
                .serialize(record)
                .expect("Failed to write CSV record");
        }
    }



}

// Routes a transaction to the appropriate engine. Transactions only ever touch a single
// user' account, so we can split based on the client ID.
fn route_transaction(
    transaction: Transaction,
    senders: &[SyncSender<Transaction>],
) -> Result<(), String> {
    let client_id = match transaction {
        Transaction::Deposit { client_id, .. }
        | Transaction::Withdrawal { client_id, .. }
        | Transaction::Dispute { client_id, .. }
        | Transaction::Resolve { client_id, .. }
        | Transaction::Chargeback { client_id, .. } => client_id,
    };

    let engine_index = (client_id as usize) % senders.len();
    senders[engine_index]
        .send(transaction)
        .map_err(|err| format!("Failed to send transaction to engine {}: {}", engine_index, err))
}
