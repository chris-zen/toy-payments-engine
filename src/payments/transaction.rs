use rust_decimal::Decimal;

/// Alias for a client ID
pub type ClientId = u16;

/// Alias for a transaction ID
pub type TransactionId = u32;

/// Representation of the transactions types supported by a payments engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Transaction {
  Deposit {
    client_id: ClientId,
    transaction_id: TransactionId,
    amount: Decimal,
  },
  Withdrawal {
    client_id: ClientId,
    transaction_id: TransactionId,
    amount: Decimal,
  },
  Dispute {
    client_id: ClientId,
    transaction_id: TransactionId,
  },
  Resolve {
    client_id: ClientId,
    transaction_id: TransactionId,
  },
  Chargeback {
    client_id: ClientId,
    transaction_id: TransactionId,
  },
}
