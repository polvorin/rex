use serde::Serialize;

use crate::types::{Amount, ClientId, Transaction, TransactionId};
use std::collections::HashMap;

// A user balance.  Holds the total balance, and the disputed and undisputed deposits.
// held amount and available balance are computed based on total balance and disputed deposits.
// this way we have a single source of truth for the available balance,
// and avoid having to update multiple fields that could lead to inconsistencies if not done carefully.
struct Balance {
    total_balance: Amount,
    is_locked: bool,
    // Assume there is few disputed deposits per account, so a vector is convenient here.
    disputed_deposits: Vec<(TransactionId, Amount)>,
    // Required for being able to process disputes. A more complete implementation would
    // need more information about the transactions, but for the sake of this exercise
    // we only need to know the amount of each deposit.
    undisputed_deposits: HashMap<TransactionId, Amount>,
}

impl Balance {
    pub fn new() -> Self {
        Balance {
            total_balance: 0,
            is_locked: false,
            disputed_deposits: vec![],
            undisputed_deposits: HashMap::new(),
        }
    }
    fn available_funds(&self) -> Amount {
        self.total_balance - self.held_funds()
    }
    fn held_funds(&self) -> Amount {
        self.disputed_deposits
            .iter()
            .fold(0, |acc, (_tx_id, amount)| acc + *amount)
    }

    fn as_report(&self, client_id: ClientId) -> BalanceReportRecord {
        // Note we compute the held funds twice here, but left for simplicity.
        BalanceReportRecord {
            client: client_id,
            available: self.available_funds() as f64 / 10000.0,
            held: self.held_funds() as f64 / 10000.0,
            total: self.total_balance as f64 / 10000.0,
            locked: self.is_locked,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BalanceReportRecord {
    client: u16,
    available: f64,
    held: f64,
    total: f64,
    locked: bool,
}

pub struct Engine {
    balances: HashMap<u16, Balance>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            balances: HashMap::new(),
        }
    }

    pub fn report(&self) -> Vec<BalanceReportRecord> {
        self.balances
            .iter()
            .map(|(client_id, balance)| balance.as_report(*client_id))
            .collect()
    }

    pub fn process_transaction(&mut self, transaction: Transaction) -> () {
        // Process the transaction and update accounts and transactions accordingly
        match transaction {
            Transaction::Deposit {
                transaction_id,
                client_id,
                amount,
            } => {
                self.process_deposit(client_id, transaction_id, amount);
            }
            Transaction::Withdrawal {
                transaction_id,
                client_id,
                amount,
            } => {
                self.with_unlocked_balance(client_id, transaction_id, |balance| {
                    process_withdrawal(balance, amount)
                });
            }
            Transaction::Dispute {
                transaction_id,
                client_id,
            } => {
                self.with_unlocked_balance(client_id, transaction_id, |balance| {
                    process_dispute(balance, transaction_id)
                });
            }
            Transaction::Resolve {
                transaction_id,
                client_id,
            } => {
                self.with_unlocked_balance(client_id, transaction_id, |balance| {
                    process_resolve(balance, transaction_id)
                });
            }
            Transaction::Chargeback {
                transaction_id,
                client_id,
            } => {
                self.with_unlocked_balance(client_id, transaction_id, |balance| {
                    process_chargeback(balance, transaction_id)
                });
            }
        }
    }

    fn process_deposit(
        &mut self,
        client_id: ClientId,
        transaction_id: TransactionId,
        amount: Amount,
    ) {
        let balance = self.balances.entry(client_id).or_insert_with(Balance::new);
        if balance.is_locked {
            log::warn!(
                "Tried to process deposit for frozen account {}: {}",
                client_id,
                transaction_id
            );
            return;
        }
        balance.total_balance += amount;
        balance.undisputed_deposits.insert(transaction_id, amount);
    }

    // Call the function `f` with the balance for `client_id` if the account is not locked
    // otherwise log a warning and skip processing.
    fn with_unlocked_balance<F>(&mut self, client_id: ClientId, transaction_id: TransactionId, f: F)
    where
        F: Fn(&mut Balance) -> Result<(), String>,
    {
        match self.balances.get_mut(&client_id) {
            Some(balance) => {
                if balance.is_locked {
                    log::warn!(
                        "Tried to process transaction for frozen account {}: {}",
                        client_id,
                        transaction_id
                    );
                    return;
                }
                if let Err(err) = f(balance) {
                    log::warn!(
                        "Error processing transaction for account {}: {} : {}",
                        client_id,
                        transaction_id,
                        err
                    );
                }
            }
            None => {
                log::warn!(
                    "Tried to process transaction for non-existent account {}: {}",
                    client_id,
                    transaction_id
                );
                return;
            }
        }
    }
}

fn process_withdrawal(balance: &mut Balance, amount: i64) -> Result<(), String> {
    if balance.available_funds() < amount {
        return Err("Insufficient funds".into());
    }
    balance.total_balance -= amount;
    Ok(())
}

fn process_dispute(balance: &mut Balance, transaction_id: u32) -> Result<(), String> {
    match balance.undisputed_deposits.remove(&transaction_id) {
        Some(deposit) => {
            balance.disputed_deposits.push((transaction_id, deposit));
            Ok(())
        }
        None => Err(format!(
            "Tried to dispute non-existing transaction {}",
            transaction_id
        )),
    }
}

fn process_resolve(balance: &mut Balance, transaction_id: u32) -> Result<(), String> {
    match balance
        .disputed_deposits
        .iter()
        .position(|&(disputed_transaction_id, _)| disputed_transaction_id == transaction_id)
    {
        Some(pos) => {
            let (_, amount) = balance.disputed_deposits.swap_remove(pos);
            balance.undisputed_deposits.insert(transaction_id, amount);
            Ok(())
        }
        None => Err(format!(
            "Tried to resolve non-existing dispute transaction {}",
            transaction_id
        )),
    }
}

fn process_chargeback(balance: &mut Balance, transaction_id: u32) -> Result<(), String> {
    match balance
        .disputed_deposits
        .iter()
        .position(|&(disputed_transaction_id, _)| disputed_transaction_id == transaction_id)
    {
        Some(pos) => {
            let (_, amount) = balance.disputed_deposits.swap_remove(pos);
            balance.total_balance -= amount;
            balance.is_locked = true;
            Ok(())
        }
        None => Err(format!(
            "Tried to chargeback non-existent dispute for transaction {}",
            transaction_id
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_deposit(client_id: u16, transaction_id: u32, amount: i64) -> Transaction {
        Transaction::Deposit {
            client_id,
            transaction_id,
            amount,
        }
    }

    fn create_withdrawal(client_id: u16, transaction_id: u32, amount: i64) -> Transaction {
        Transaction::Withdrawal {
            client_id,
            transaction_id,
            amount,
        }
    }

    fn create_dispute(client_id: u16, transaction_id: u32) -> Transaction {
        Transaction::Dispute {
            client_id,
            transaction_id,
        }
    }

    fn create_resolve(client_id: u16, transaction_id: u32) -> Transaction {
        Transaction::Resolve {
            client_id,
            transaction_id,
        }
    }

    fn create_chargeback(client_id: u16, transaction_id: u32) -> Transaction {
        Transaction::Chargeback {
            client_id,
            transaction_id,
        }
    }

    #[test]
    fn test_three_deposits_one_disputed() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 3 deposits: $100, $200, $300
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));
        engine.process_transaction(create_deposit(client_id, 3, 3000000));

        // Dispute deposit #2 ($200)
        engine.process_transaction(create_dispute(client_id, 2));

        let report = engine.report();
        assert_eq!(report.len(), 1);
        let record = &report[0];
        assert_eq!(record.client, client_id);
        assert_eq!(record.total, 600.0); // $100 + $200 + $300
        assert_eq!(record.held, 200.0); // $200 held (disputed)
        assert_eq!(record.available, 400.0); // $600 - $200
        assert!(!record.locked);
    }

    #[test]
    fn test_three_deposits_one_disputed_and_resolved() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 3 deposits: $100, $200, $300
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));
        engine.process_transaction(create_deposit(client_id, 3, 3000000));

        // Dispute deposit #2 ($200)
        engine.process_transaction(create_dispute(client_id, 2));

        let report = engine.report();
        assert_eq!(report[0].held, 200.0);
        assert_eq!(report[0].available, 400.0);

        // Resolve deposit #2
        engine.process_transaction(create_resolve(client_id, 2));

        let report = engine.report();
        assert_eq!(report.len(), 1);
        let record = &report[0];
        assert_eq!(record.client, client_id);
        assert_eq!(record.total, 600.0); // $100 + $200 + $300
        assert_eq!(record.held, 0.0); // resolved, no held funds
        assert_eq!(record.available, 600.0); // all available again
        assert!(!record.locked);
    }

    #[test]
    fn test_three_deposits_one_disputed_and_chargeback() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 3 deposits: $100, $200, $300
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));
        engine.process_transaction(create_deposit(client_id, 3, 3000000));

        // Dispute deposit #2 ($200)
        engine.process_transaction(create_dispute(client_id, 2));

        let report = engine.report();
        assert_eq!(report[0].held, 200.0);

        // Chargeback deposit #2
        engine.process_transaction(create_chargeback(client_id, 2));

        let report = engine.report();
        assert_eq!(report.len(), 1);
        let record = &report[0];
        assert_eq!(record.client, client_id);
        assert_eq!(record.total, 400.0); // $600 - $200 chargeback
        assert_eq!(record.held, 0.0); // removed from held
        assert_eq!(record.available, 400.0); // $400 - $0
        assert!(record.locked); // locked after chargeback
    }

    #[test]
    fn test_three_deposits_all_disputed_and_chargeback() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 3 deposits: $100, $200, $300
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));
        engine.process_transaction(create_deposit(client_id, 3, 3000000));

        // Dispute all 3
        engine.process_transaction(create_dispute(client_id, 1));
        engine.process_transaction(create_dispute(client_id, 2));
        engine.process_transaction(create_dispute(client_id, 3));

        let report = engine.report();
        assert_eq!(report[0].total, 600.0);
        assert_eq!(report[0].held, 600.0);
        assert_eq!(report[0].available, 0.0);

        // Chargeback all 3 - only the first one succeeds because account locks after first chargeback
        engine.process_transaction(create_chargeback(client_id, 1));
        engine.process_transaction(create_chargeback(client_id, 2)); // blocked (account locked)
        engine.process_transaction(create_chargeback(client_id, 3)); // blocked (account locked)

        let report = engine.report();
        assert_eq!(report.len(), 1);
        let record = &report[0];
        assert_eq!(record.client, client_id);
        assert_eq!(record.total, 500.0); // $600 - $100 (only first chargeback succeeds)
        assert_eq!(record.held, 500.0); // tx 2 ($200) and tx 3 ($300) still disputed
        assert_eq!(record.available, 0.0); // $500 - $500
        assert!(record.locked); // locked after first chargeback
    }

    #[test]
    fn test_three_deposits_withdraw_all_then_dispute() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 3 deposits: $100, $200, $300
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));
        engine.process_transaction(create_deposit(client_id, 3, 3000000));

        // Withdraw entire balance ($600)
        engine.process_transaction(create_withdrawal(client_id, 4, 6000000));

        let report = engine.report();
        assert_eq!(report[0].total, 0.0);
        assert_eq!(report[0].available, 0.0);

        // Dispute deposit #2 ($200)
        engine.process_transaction(create_dispute(client_id, 2));

        let report = engine.report();
        assert_eq!(report.len(), 1);
        let record = &report[0];
        assert_eq!(record.client, client_id);
        assert_eq!(record.total, 0.0); // total unchanged
        assert_eq!(record.held, 200.0); // $200 now held
        assert_eq!(record.available, -200.0); // $0 - $200 = negative available
        assert!(!record.locked);
    }

    #[test]
    fn test_three_deposits_withdraw_all_then_dispute_and_chargeback() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 3 deposits: $100, $200, $300
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));
        engine.process_transaction(create_deposit(client_id, 3, 3000000));

        // Withdraw entire balance ($600)
        engine.process_transaction(create_withdrawal(client_id, 4, 6000000));

        let report = engine.report();
        assert_eq!(report[0].total, 0.0);
        assert_eq!(report[0].available, 0.0);

        // Dispute deposit #2 ($200)
        engine.process_transaction(create_dispute(client_id, 2));

        let report = engine.report();
        assert_eq!(report[0].held, 200.0);
        assert_eq!(report[0].available, -200.0);

        // Chargeback deposit #2
        engine.process_transaction(create_chargeback(client_id, 2));

        let report = engine.report();
        assert_eq!(report.len(), 1);
        let record = &report[0];
        assert_eq!(record.client, client_id);
        assert_eq!(record.total, -200.0); // $0 - $200 chargeback
        assert_eq!(record.held, 0.0); // removed from held
        assert_eq!(record.available, -200.0); // -$200 - $0
        assert!(record.locked); // locked after chargeback
    }

    #[test]
    fn test_locked_account_cannot_receive_deposit() {
        let mut engine = Engine::new();
        let client_id = 1;

        // Make a deposit and lock the account via chargeback
        engine.process_transaction(create_deposit(client_id, 1, 1000000)); // $100
        engine.process_transaction(create_dispute(client_id, 1));
        engine.process_transaction(create_chargeback(client_id, 1));

        let report = engine.report();
        assert!(report[0].locked); // account is now locked
        assert_eq!(report[0].total, 0.0); // chargeback removed the $100

        // Try to deposit on locked account
        engine.process_transaction(create_deposit(client_id, 2, 500000)); // $50

        let report = engine.report();
        // Deposit should be blocked on locked account
        assert_eq!(report[0].total, 0.0); // total should remain $0
        assert!(report[0].locked);
    }

    #[test]
    fn test_locked_account_cannot_withdraw() {
        let mut engine = Engine::new();
        let client_id = 1;

        // Make deposits
        engine.process_transaction(create_deposit(client_id, 1, 1000000)); // $100
        engine.process_transaction(create_deposit(client_id, 2, 2000000)); // $200

        // Lock the account via dispute + chargeback on tx#1
        engine.process_transaction(create_dispute(client_id, 1));
        engine.process_transaction(create_chargeback(client_id, 1));

        let report = engine.report();
        assert!(report[0].locked); // account is now locked
        assert_eq!(report[0].total, 200.0); // $300 - $100 chargeback = $200

        // Try to withdraw on locked account
        engine.process_transaction(create_withdrawal(client_id, 3, 1000000)); // withdraw $100

        let report = engine.report();
        // Withdrawal should be blocked on locked account
        assert_eq!(report[0].total, 200.0); // total should remain $200
        assert!(report[0].locked);
    }

    #[test]
    fn test_withdrawal_larger_than_available_funds_rejected() {
        let mut engine = Engine::new();
        let client_id = 1;

        // Deposit $100
        engine.process_transaction(create_deposit(client_id, 1, 1000000));

        let report = engine.report();
        assert_eq!(report[0].total, 100.0);
        assert_eq!(report[0].available, 100.0);

        // Try to withdraw $200 (more than available)
        engine.process_transaction(create_withdrawal(client_id, 2, 2000000));

        let report = engine.report();
        // Withdrawal should be rejected, balance unchanged
        assert_eq!(report[0].total, 100.0);
        assert_eq!(report[0].available, 100.0);
    }

    #[test]
    fn test_withdrawal_allowed_with_held_funds_if_available_sufficient() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 3 deposits: $100, $200, $300
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));
        engine.process_transaction(create_deposit(client_id, 3, 3000000));

        // Dispute deposit #3 ($300) → held = $300, available = $300
        engine.process_transaction(create_dispute(client_id, 3));

        let report = engine.report();
        assert_eq!(report[0].total, 600.0);
        assert_eq!(report[0].held, 300.0);
        assert_eq!(report[0].available, 300.0);

        // Withdraw $200 (less than available $300, even though total is $600)
        engine.process_transaction(create_withdrawal(client_id, 4, 2000000));

        let report = engine.report();
        assert_eq!(report[0].total, 400.0); // $600 - $200 withdrawal
        assert_eq!(report[0].held, 300.0); // held unchanged
        assert_eq!(report[0].available, 100.0); // $400 - $300 held
    }

    #[test]
    fn test_withdrawal_exactly_available_funds_allowed() {
        let mut engine = Engine::new();
        let client_id = 1;

        // 2 deposits: $100, $200
        engine.process_transaction(create_deposit(client_id, 1, 1000000));
        engine.process_transaction(create_deposit(client_id, 2, 2000000));

        // Dispute deposit #2 ($200) → held = $200, available = $100
        engine.process_transaction(create_dispute(client_id, 2));

        let report = engine.report();
        assert_eq!(report[0].total, 300.0);
        assert_eq!(report[0].held, 200.0);
        assert_eq!(report[0].available, 100.0);

        // Withdraw exactly $100 (equal to available)
        engine.process_transaction(create_withdrawal(client_id, 3, 1000000));

        let report = engine.report();
        assert_eq!(report[0].total, 200.0); // $300 - $100 withdrawal
        assert_eq!(report[0].held, 200.0); // held unchanged
        assert_eq!(report[0].available, 0.0); // $200 - $200 held
    }
}
