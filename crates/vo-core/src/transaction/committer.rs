//! TransactionCommitter trait for committing batches.

use crate::db_writer_message::DbWriterMessage;
use crate::transaction::TransactionError;

pub trait TransactionCommitter {
    fn commit_batch(&self, messages: Vec<DbWriterMessage>) -> Result<(), TransactionError>;
}
