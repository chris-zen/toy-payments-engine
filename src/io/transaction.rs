use std::convert::TryFrom;

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::payments;

/// The types of transactions supported by the reader
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
  Deposit,
  Withdrawal,
  Dispute,
  Resolve,
  Chargeback,
}

/// A deserializable transaction
#[derive(Debug, Deserialize)]
pub struct Transaction {
  #[serde(rename = "type")]
  kind: TransactionType,

  #[serde(rename = "client")]
  client_id: u16,

  #[serde(rename = "tx")]
  transaction_id: u32,

  amount: Option<Decimal>,
}

impl TryFrom<Transaction> for payments::Transaction {
  type Error = anyhow::Error;

  fn try_from(transaction: Transaction) -> Result<Self, Self::Error> {
    match transaction.kind {
      TransactionType::Deposit => transaction
        .amount
        .map(|amount| payments::Transaction::Deposit {
          client_id: transaction.client_id,
          transaction_id: transaction.transaction_id,
          amount,
        })
        .ok_or_else(|| anyhow::anyhow!("Missing amount")),
      TransactionType::Withdrawal => transaction
        .amount
        .map(|amount| payments::Transaction::Withdrawal {
          client_id: transaction.client_id,
          transaction_id: transaction.transaction_id,
          amount,
        })
        .ok_or_else(|| anyhow::anyhow!("Missing amount")),
      TransactionType::Dispute => Ok(payments::Transaction::Dispute {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
      }),
      TransactionType::Resolve => Ok(payments::Transaction::Resolve {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
      }),
      TransactionType::Chargeback => Ok(payments::Transaction::Chargeback {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
      }),
    }
  }
}

#[cfg(test)]
mod tests {

  use rust_decimal_macros::dec;

  use super::*;

  #[test]
  fn payments_transaction_try_from_success() {
    let cases = vec![
      (
        Transaction {
          kind: TransactionType::Deposit,
          client_id: 1,
          transaction_id: 101,
          amount: Some(dec!(100)),
        },
        payments::Transaction::Deposit {
          client_id: 1,
          transaction_id: 101,
          amount: dec!(100),
        },
      ),
      (
        Transaction {
          kind: TransactionType::Withdrawal,
          client_id: 2,
          transaction_id: 102,
          amount: Some(dec!(200)),
        },
        payments::Transaction::Withdrawal {
          client_id: 2,
          transaction_id: 102,
          amount: dec!(200),
        },
      ),
      (
        Transaction {
          kind: TransactionType::Dispute,
          client_id: 3,
          transaction_id: 103,
          amount: None,
        },
        payments::Transaction::Dispute {
          client_id: 3,
          transaction_id: 103,
        },
      ),
      (
        Transaction {
          kind: TransactionType::Resolve,
          client_id: 4,
          transaction_id: 104,
          amount: None,
        },
        payments::Transaction::Resolve {
          client_id: 4,
          transaction_id: 104,
        },
      ),
      (
        Transaction {
          kind: TransactionType::Chargeback,
          client_id: 5,
          transaction_id: 105,
          amount: None,
        },
        payments::Transaction::Chargeback {
          client_id: 5,
          transaction_id: 105,
        },
      ),
    ];

    for (input, expected) in cases {
      let tx = payments::Transaction::try_from(input);
      assert!(tx.is_ok());
      assert_eq!(tx.unwrap(), expected);
    }
  }

  #[test]
  fn payments_transaction_try_from_missing_amount() {
    assert!(payments::Transaction::try_from(Transaction {
      kind: TransactionType::Deposit,
      client_id: 1,
      transaction_id: 101,
      amount: None,
    })
    .is_err());

    assert!(payments::Transaction::try_from(Transaction {
      kind: TransactionType::Withdrawal,
      client_id: 1,
      transaction_id: 101,
      amount: None,
    })
    .is_err());
  }
}
