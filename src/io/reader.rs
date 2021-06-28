use anyhow::Result;
use tokio::io::AsyncRead;
use tokio_stream::{Stream, StreamExt};

use crate::payments::Transaction;

/// Interface to read transactions from an external source
pub trait TransactionsReader {
  /// Read transactions and return an [`Stream`] of possibly successful transactions.
  /// Each item yielded by the stream is either `Ok` if the transaction was read successfully,
  /// or `Err` if there was any kind of problem (like wrong format).
  fn read_transactions<'a>(
    &'a mut self,
  ) -> Box<dyn Stream<Item = Result<Transaction>> + Unpin + 'a>;
}

/// Implementation of [`TransactionsReader`] for the CSV format.
pub struct CsvTransactionsReader<R>(R);

impl<R> CsvTransactionsReader<R>
where
  R: AsyncRead + Unpin + Send + Sync,
{
  pub fn new(reader: R) -> Self {
    Self(reader)
  }
}

impl<R> TransactionsReader for CsvTransactionsReader<R>
where
  R: AsyncRead + Unpin + Send + Sync,
{
  fn read_transactions<'a>(
    &'a mut self,
  ) -> Box<dyn Stream<Item = Result<Transaction>> + Unpin + 'a> {
    Box::new(
      csv_async::AsyncDeserializer::from_reader(&mut self.0)
        .into_deserialize::<super::transaction::Transaction>()
        .map(|maybe_transaction| {
          maybe_transaction
            .map_err(anyhow::Error::from)
            .map(Transaction::from)
        }),
    )
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use indoc::indoc;
  use rust_decimal_macros::dec;

  #[tokio::test]
  async fn read_transactions_with_format_errors() {
    let input = indoc! { "
      type,client,tx,amount
      deposit
      withdrawal,2,102,10.5

      deposit,3,202,1000
      unknown,1,2,3
    " }
    .as_bytes();

    let mut reader = CsvTransactionsReader::new(input);

    let transactions = reader
      .read_transactions()
      .map(|tx| tx.map(|_| "ok").unwrap_or_else(|_| "err"))
      .collect::<Vec<&str>>()
      .await;

    assert_eq!(transactions, vec!["err", "ok", "ok", "err"])
  }

  #[tokio::test]
  async fn read_transactions_success() {
    let input = indoc! { "
      type,client,tx,amount
      deposit,1,101,100
      withdrawal,2,102,10.5
    " }
    .as_bytes();

    let mut reader = CsvTransactionsReader::new(input);

    let transactions = reader
      .read_transactions()
      .map(|tx| tx.map_err(|err| err.to_string()))
      .collect::<Vec<Result<Transaction, String>>>()
      .await;

    assert_eq!(
      transactions,
      vec![
        Ok(Transaction::Deposit {
          client_id: 1,
          transaction_id: 101,
          amount: dec!(100),
        }),
        Ok(Transaction::Withdrawal {
          client_id: 2,
          transaction_id: 102,
          amount: dec!(10.5),
        }),
      ]
    )
  }
}
