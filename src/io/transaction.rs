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

  amount: Decimal,
}

impl From<Transaction> for payments::Transaction {
  /// Conversion from a deserializable Transaction into one that can be used by the domain logic.
  fn from(transaction: Transaction) -> Self {
    match transaction.kind {
      TransactionType::Deposit => payments::Transaction::Deposit {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
        amount: transaction.amount,
      },
      TransactionType::Withdrawal => payments::Transaction::Withdrawal {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
        amount: transaction.amount,
      },
      TransactionType::Dispute => payments::Transaction::Dispute {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
      },
      TransactionType::Resolve => payments::Transaction::Resolve {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
      },
      TransactionType::Chargeback => payments::Transaction::Chargeback {
        client_id: transaction.client_id,
        transaction_id: transaction.transaction_id,
      },
    }
  }
}

#[cfg(test)]
mod tests {

  use rust_decimal_macros::dec;

  use super::*;

  #[test]
  fn payments_transaction_from() {
    let cases = vec![
      (
        Transaction {
          kind: TransactionType::Deposit,
          client_id: 1,
          transaction_id: 101,
          amount: dec!(100),
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
          amount: dec!(200),
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
          amount: dec!(0),
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
          amount: dec!(0),
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
          amount: dec!(0),
        },
        payments::Transaction::Chargeback {
          client_id: 5,
          transaction_id: 105,
        },
      ),
    ];

    for (input, expected) in cases {
      assert_eq!(payments::Transaction::from(input), expected)
    }
  }
}
