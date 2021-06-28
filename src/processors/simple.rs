use anyhow::Result;
use tokio_stream::StreamExt;

use crate::io::{AccountsReportWriter, TransactionsReader};
use crate::payments::PaymentsEngine;

/// This is a simple processor of payments that
/// - reads transactions from a [`TransactionsReader`]
/// - processes payments using a [`PaymentsEngine`]
/// - writes a report including accounts state using a [`AccountsReportWriter`]
///
/// The idea is that all those components can be replaced with different implementations.
///
/// This processor tries to be as resilient as possible, meaning that:
/// - errors from the transactions reader will be skipped
/// - errors from the payments engine will be skipped
///
/// In the reality, those errors should be instrumented as metrics and/or logs that can be tracked and alerted on,
/// and the errors happening in the payments engine could be reported as events to a fraud detection system.
///
/// Following similar ideas, and thanks the way that the architecture have been designed,
/// it shouldn't be too difficult to write other kind of processors like:
/// - An HTTP streaming processor, where transactions are sent as a request and accounts reports returned as an stream
/// - A partitioned multi-threaded processor, where multiple threads, everyone with its own instance of a payments engine,
///   process transactions in parallel. The key idea would be to partition the transactions by a uniform hash of the client_id,
///   and then send them to the corresponding thread using a channel. The multi-threaded logic could be implemented using
///   the [`PaymentsEngine`] trait so this simple processor could still be used.
///
pub async fn run<R, P, W>(
  mut transactions_reader: R,
  mut payments_engine: P,
  mut accounts_report_writer: W,
) -> Result<()>
where
  R: TransactionsReader,
  P: PaymentsEngine,
  W: AccountsReportWriter,
{
  let mut transactions = transactions_reader.read_transactions();

  while let Some(maybe_transaction) = transactions.next().await {
    if let Ok(transaction) = maybe_transaction {
      payments_engine.process(transaction).await.ok();
    }
  }

  accounts_report_writer
    .write_accounts_report(payments_engine.accounts_report())
    .await
}

#[cfg(test)]
mod test {

  use async_trait::async_trait;
  use mock_it::Mock;
  use rust_decimal_macros::dec;
  use tokio_stream::Stream;

  use super::*;
  use crate::payments::{
    AccountReport, AccountsReportIter, EngineResult, PaymentsEngine, PaymentsEngineError,
    Transaction,
  };

  #[tokio::test]
  async fn run_successfully() {
    let transaction1 = Transaction::Deposit {
      client_id: 1,
      transaction_id: 102,
      amount: dec!(-10),
    };

    let transaction2 = Transaction::Deposit {
      client_id: 1,
      transaction_id: 101,
      amount: dec!(10),
    };

    let transactions_reader = create_transaction_reader_mock(vec![
      Err("some failure".to_string()),
      Ok(transaction1.clone()),
      Ok(transaction2.clone()),
    ]);

    let account_reports = vec![AccountReport::new(1, dec!(10), dec!(0), dec!(10), false)];

    let payments_engine = create_payments_engine_mock(
      vec![
        (transaction1, Err(PaymentsEngineError::NegativeAmount)),
        (transaction2, Ok(())),
      ],
      account_reports.clone(),
    );

    let accounts_report_writer = create_accounts_report_writer_mock(account_reports);

    let result = run(transactions_reader, payments_engine, accounts_report_writer).await;

    assert!(result.is_ok())
  }

  mockall::mock! {
    TestTransactionReader {}
    impl TransactionsReader for TestTransactionReader {
      fn read_transactions<'a>(
        &'a mut self,
      ) -> Box<dyn Stream<Item = Result<Transaction>> + Unpin + 'a>;
    }
  }

  fn create_transaction_reader_mock(
    transactions: Vec<Result<Transaction, String>>,
  ) -> MockTestTransactionReader {
    let mut transactions_reader = MockTestTransactionReader::new();
    transactions_reader
      .expect_read_transactions()
      .returning(move || {
        Box::new(tokio_stream::iter(
          transactions
            .clone()
            .into_iter()
            .map(|result| result.map_err(|err| anyhow::anyhow!(err))),
        ))
      });
    transactions_reader
  }

  mockall::mock! {
    TestPaymentsEngine {}
    #[async_trait]
    impl PaymentsEngine for TestPaymentsEngine {
      async fn process(&mut self, transaction: Transaction) -> EngineResult<()>;
      fn accounts_report(&self) -> AccountsReportIter<'_>;
    }
  }

  fn create_payments_engine_mock(
    transactions: Vec<(Transaction, Result<(), PaymentsEngineError>)>,
    account_reports: Vec<AccountReport>,
  ) -> MockTestPaymentsEngine {
    let mut payments_engine = MockTestPaymentsEngine::new();
    for (transaction, result) in transactions {
      payments_engine
        .expect_process()
        .with(mockall::predicate::eq(transaction))
        .return_const(result);
    }
    payments_engine
      .expect_accounts_report()
      .returning(move || AccountsReportIter::new(account_reports.clone().into_iter()));
    payments_engine
  }

  // I had to use `mock-it` for this specific mock because `mockall` was failing.
  // More information here: https://github.com/asomers/mockall/issues/299

  pub struct MockTestAccountsReportWriter {
    write_accounts_report: Mock<Vec<AccountReport>, Result<(), String>>,
  }

  impl MockTestAccountsReportWriter {
    pub fn new() -> Self {
      Self {
        write_accounts_report: Mock::new(Err("no rule satisfied".to_string())),
      }
    }
  }

  #[async_trait(?Send)]
  impl AccountsReportWriter for MockTestAccountsReportWriter {
    async fn write_accounts_report<'a, T>(&'a mut self, report: T) -> anyhow::Result<()>
    where
      T: Iterator<Item = AccountReport> + 'a,
    {
      self
        .write_accounts_report
        .called(report.collect())
        .map_err(|err| anyhow::anyhow!(err))
    }
  }

  fn create_accounts_report_writer_mock(
    account_reports: Vec<AccountReport>,
  ) -> MockTestAccountsReportWriter {
    let accounts_report_writer = MockTestAccountsReportWriter::new();
    accounts_report_writer
      .write_accounts_report
      .given(account_reports)
      .will_return(Ok(()));
    accounts_report_writer
  }
}
