use std::convert::TryFrom;

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
      csv_async::AsyncReaderBuilder::new()
        .flexible(true)
        .create_reader(&mut self.0)
        .into_records()
        .map(|maybe_record| {
          maybe_record
            .and_then(|mut record| {
              record.trim();
              if record.len() == 3 {
                record.push_field("");
              }
              record.deserialize::<super::transaction::Transaction>(None)
            })
            .map_err(anyhow::Error::from)
            .and_then(Transaction::try_from)
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
      type,      client,     tx,        amount
      deposit
      deposit,,,
       withdrawal,    2,    102,       
      withdrawal,    2,    103

      deposit  ,      3,    202 ,
      deposit  ,      3,    203
      unknown,1,2,3
    " }
    .as_bytes();

    let mut reader = CsvTransactionsReader::new(input);

    let transactions = reader
      .read_transactions()
      .map(|tx| tx.map(|_| "ok").unwrap_or_else(|_| "err"))
      .collect::<Vec<&str>>()
      .await;

    assert_eq!(transactions.iter().filter(|v| **v == "err").count(), 7);
    assert_eq!(transactions.iter().filter(|v| **v == "ok").count(), 0);
  }

  #[tokio::test]
  async fn read_transactions_success() {
    let input = indoc! { "
      type,       client,   tx,  amount
      deposit,         1,  101,     100
       withdrawal,     2,  102,    10.5
      dispute,         1,  103,
      resolve,         1,  104
      chargeback,      1,  105,
      dispute,         1,  106, 10.0
      resolve,         1,  107, 1.0
      chargeback,      1,  108, 10.0
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
        Ok(Transaction::Dispute {
          client_id: 1,
          transaction_id: 103,
        }),
        Ok(Transaction::Resolve {
          client_id: 1,
          transaction_id: 104,
        }),
        Ok(Transaction::Chargeback {
          client_id: 1,
          transaction_id: 105,
        }),
        Ok(Transaction::Dispute {
          client_id: 1,
          transaction_id: 106,
        }),
        Ok(Transaction::Resolve {
          client_id: 1,
          transaction_id: 107,
        }),
        Ok(Transaction::Chargeback {
          client_id: 1,
          transaction_id: 108,
        })
      ]
    )
  }
}
