mod io;
mod payments;
mod processors;

use anyhow::Result;
use tokio::io::AsyncRead;

use crate::io::{CsvAccountsReportWriter, CsvTransactionsReader};
use payments::InMemoryPaymentsEngine;

#[tokio::main]
async fn main() -> Result<()> {
  let reader = get_transactions_async_read().await?;
  let transactions_reader = CsvTransactionsReader::new(reader);
  let payments_engine = InMemoryPaymentsEngine::new();
  let accounts_report_writer = CsvAccountsReportWriter::new(tokio::io::stdout());

  processors::simple::run(transactions_reader, payments_engine, accounts_report_writer).await
}

type TransactionsAsyncRead = Box<dyn AsyncRead + Unpin + Send + Sync>;

/// This allows to use either a file if the path is specified in the command line,
/// or the stdin otherwise, which might be more convenient for pipe the data.
async fn get_transactions_async_read() -> Result<TransactionsAsyncRead> {
  match std::env::args().nth(1) {
    Some(path) => tokio::fs::File::open(path)
      .await
      .map(|file| Box::new(file) as TransactionsAsyncRead)
      .map_err(anyhow::Error::from),
    None => Ok(Box::new(tokio::io::stdin()) as TransactionsAsyncRead),
  }
}
