use serde::Deserialize;

pub type TransactionId = u32;
pub type ClientId = u16;

// Balances are stored as i64, and represented as 1/10000.
// For example, a balance of $1.2324 would be stored as 12324.
pub type Amount = i64;

// Transactions as read from CSV
#[derive(Debug, Deserialize)]
pub struct RawTransaction {
    #[serde(rename = "type")]
    pub tx_type: String,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Option<f64>,
}

impl RawTransaction {
    pub fn parse_amount(&self) -> Option<Amount> {
        self.amount.map(|a| a.mul_add(10000.0, 0.5) as i64)
    }

    pub fn into_transaction(self) -> Result<Transaction, String> {
        match self.tx_type.as_str() {
            "deposit" => Ok(Transaction::Deposit {
                transaction_id: self.tx,
                client_id: self.client,
                amount: self
                    .parse_amount()
                    .ok_or_else(|| "Amount is required for deposit".to_string())?,
            }),
            "withdrawal" => Ok(Transaction::Withdrawal {
                transaction_id: self.tx,
                client_id: self.client,
                amount: self
                    .parse_amount()
                    .ok_or_else(|| "Amount is required for withdrawal".to_string())?,
            }),
            "dispute" => Ok(Transaction::Dispute {
                transaction_id: self.tx,
                client_id: self.client,
            }),
            "resolve" => Ok(Transaction::Resolve {
                transaction_id: self.tx,
                client_id: self.client,
            }),
            "chargeback" => Ok(Transaction::Chargeback {
                transaction_id: self.tx,
                client_id: self.client,
            }),
            other => Err(format!("Unknown transaction type: {}", other)),
        }
    }
}

// Internal representation of transactions used by the engine.
// See https://github.com/BurntSushi/rust-csv/pull/231 for rationale behind this separation.
// Didn't want to complicate the CSV parsing code in the exercise just for perf reasons.
#[derive(Debug)]
pub enum Transaction {
    Deposit {
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Amount,
    },
    Withdrawal {
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Amount,
    },
    Dispute {
        transaction_id: TransactionId,
        client_id: ClientId,
    },
    Resolve {
        transaction_id: TransactionId,
        client_id: ClientId,
    },
    Chargeback {
        transaction_id: TransactionId,
        client_id: ClientId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse_csv(csv_input: &str) -> Vec<Transaction> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(Cursor::new(csv_input));

        let mut transactions = Vec::new();
        for result in rdr.deserialize() {
            let raw: RawTransaction = result.expect("Failed to deserialize CSV record");
            let tx = raw
                .into_transaction()
                .expect("Failed to convert to Transaction");
            transactions.push(tx);
        }
        transactions
    }

    #[test]
    fn test_parse_deposit() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,100.5000\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Deposit {
                transaction_id,
                client_id,
                amount,
            } => {
                assert_eq!(*transaction_id, 1001);
                assert_eq!(*client_id, 1);
                assert_eq!(*amount, 1_005_000);
            }
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_parse_withdrawal() {
        let csv = "type,client,tx,amount\nwithdrawal,2,2001,50.0000\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Withdrawal {
                transaction_id,
                client_id,
                amount,
            } => {
                assert_eq!(*transaction_id, 2001);
                assert_eq!(*client_id, 2);
                assert_eq!(*amount, 500_000);
            }
            _ => panic!("Expected Withdrawal"),
        }
    }

    #[test]
    fn test_parse_dispute() {
        let csv = "type,client,tx,amount\ndispute,1,1001,0.0000\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Dispute {
                transaction_id,
                client_id,
            } => {
                assert_eq!(*transaction_id, 1001);
                assert_eq!(*client_id, 1);
            }
            _ => panic!("Expected Dispute"),
        }
    }

    #[test]
    fn test_parse_resolve() {
        let csv = "type,client,tx,amount\nresolve,1,1001,0.0000\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Resolve {
                transaction_id,
                client_id,
            } => {
                assert_eq!(*transaction_id, 1001);
                assert_eq!(*client_id, 1);
            }
            _ => panic!("Expected Resolve"),
        }
    }

    #[test]
    fn test_parse_chargeback() {
        let csv = "type,client,tx,amount\nchargeback,1,1001,0.0000\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Chargeback {
                transaction_id,
                client_id,
            } => {
                assert_eq!(*transaction_id, 1001);
                assert_eq!(*client_id, 1);
            }
            _ => panic!("Expected Chargeback"),
        }
    }

    #[test]
    fn test_amount_integer_no_decimals() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,100\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 1_000_000),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_amount_one_decimal_place() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,1.1\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 11_000),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_amount_two_decimal_places() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,1.23\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 12_300),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_amount_three_decimal_places() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,1.234\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 12_340),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_amount_four_decimal_places() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,1.2324\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 12_324),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_amount_five_decimal_places() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,1.23456\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => {
                // 1.23456 * 10000 = 12345.6, rounded = 12346
                assert_eq!(*amount, 12_346);
            }
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_amount_zero() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,0\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 0),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_amount_zero_decimal() {
        let csv = "type,client,tx,amount\ndeposit,1,1001,0.0000\n";
        let txs = parse_csv(csv);
        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 0),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_multiple_transactions() {
        let csv =
            "type,client,tx,amount\ndeposit,1,1001,100\nwithdrawal,1,1002,50.5\ndispute,1,1001,0\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 3);

        match &txs[0] {
            Transaction::Deposit { amount, .. } => assert_eq!(*amount, 1_000_000),
            _ => panic!("Expected Deposit"),
        }
        match &txs[1] {
            Transaction::Withdrawal { amount, .. } => assert_eq!(*amount, 505_000),
            _ => panic!("Expected Withdrawal"),
        }
        assert!(matches!(txs[2], Transaction::Dispute { .. }));
    }

    #[test]
    fn test_unknown_transaction_type() {
        let csv = "type,client,tx,amount\ninvalid,1,1001,100\n";
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(Cursor::new(csv));

        for result in rdr.deserialize() {
            let raw: RawTransaction = result.expect("Failed to deserialize");
            let err = raw
                .into_transaction()
                .expect_err("Expected error for unknown type");
            assert!(err.contains("Unknown transaction type"));
        }
    }

    #[test]
    fn test_dispute_missing_amount() {
        let csv = "type,client,tx,amount\ndispute,1,1001,\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Dispute {
                transaction_id,
                client_id,
            } => {
                assert_eq!(*transaction_id, 1001);
                assert_eq!(*client_id, 1);
            }
            _ => panic!("Expected Dispute"),
        }
    }

    #[test]
    fn test_resolve_missing_amount() {
        let csv = "type,client,tx,amount\nresolve,2,2001,\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Resolve {
                transaction_id,
                client_id,
            } => {
                assert_eq!(*transaction_id, 2001);
                assert_eq!(*client_id, 2);
            }
            _ => panic!("Expected Resolve"),
        }
    }

    #[test]
    fn test_chargeback_missing_amount() {
        let csv = "type,client,tx,amount\nchargeback,3,3001,\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Chargeback {
                transaction_id,
                client_id,
            } => {
                assert_eq!(*transaction_id, 3001);
                assert_eq!(*client_id, 3);
            }
            _ => panic!("Expected Chargeback"),
        }
    }
    #[test]
    fn test_chargeback_cero_amount() {
        let csv = "type,client,tx,amount\nchargeback,3,3001,0\n";
        let txs = parse_csv(csv);
        assert_eq!(txs.len(), 1);
        match &txs[0] {
            Transaction::Chargeback {
                transaction_id,
                client_id,
            } => {
                assert_eq!(*transaction_id, 3001);
                assert_eq!(*client_id, 3);
            }
            _ => panic!("Expected Chargeback"),
        }
    }
}
