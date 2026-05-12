use crate::types::{Transaction, DepositRecord};
pub struct Engine {
}

impl Engine { 
    pub fn new() -> Self {
        Engine { }
    }


    pub fn process_transaction(&self, transaction: Transaction)  -> () {
        // Process the transaction and update accounts accordingly
        match transaction {
            Transaction::Deposit(deposit) => {
                println!("Processing deposit: {:?}", deposit);
            }
            Transaction::Withdrawal { transaction_id, client_id, amount } => {
                println!("Processing withdrawal: {:?}", transaction_id);
            }
            Transaction::Dispute { transaction_id, client_id } => {
                println!("Processing dispute: {:?}", transaction_id );
            }
            Transaction::Resolve { transaction_id, client_id } => {
                println!("Processing resolve: {:?}", transaction_id);
            }
            Transaction::Chargeback { transaction_id, client_id } => {
                println!("Processing chargeback: {:?}", transaction_id);
            }
        }
    }
}
