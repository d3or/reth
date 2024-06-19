use crate::tree::ExecutedBlock;
use futures::{ready, FutureExt};
use reth_primitives::B256;
use reth_provider::ProviderFactory;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    sync::{mpsc::Receiver, oneshot},
    task::{spawn_blocking, JoinHandle},
};

/// Writes parts of reth's in memory tree state to the database.
pub struct Persistence<DB> {
    /// The db / static file provider to use
    provider: ProviderFactory<DB>,
    /// Incoming requests to persist stuff
    incoming: Receiver<PersistenceAction>,
    /// The current active thread
    active_writer_thread: Option<JoinHandle<()>>,
}

impl<Writer> Persistence<Writer> {
    // TODO: initialization
    /// Writes the cloned tree state to the database
    fn write(&mut self, blocks: Vec<ExecutedBlock>) {
        todo!("implement this")
    }

    /// Removes the blocks above the give block number from the database, returning them.
    fn remove_blocks_above(&mut self, block_number: u64) -> Vec<ExecutedBlock> {
        todo!("implement this")
    }
}

impl<Writer> Persistence<Writer>
where
    Writer: Unpin,
{
    /// Internal method to poll the persistence task. This returns [`None`] if the channel for
    /// incoming [`PersistenceAction`]s is closed.
    #[tracing::instrument(level = "debug", name = "Persistence::poll", skip(self, cx))]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<PersistenceOutput>> {
        let this = self.get_mut();

        if let Some(handle) = this.active_writer_thread {
            // if pending we just keep waiting until we can do something
            if let Err(_err) = ready!(handle.poll_unpin(cx)) {
                todo!("handle errors");
            }
        }

        let action = match ready!(this.incoming.poll_recv(cx)) {
            None => return Poll::Ready(None),
            Some(action) => action,
        };

        match action {
            PersistenceAction::RemoveBlocksAbove(new_tip_num) => {
                let (sender, receiver) = oneshot::channel();

                // spawn blocking so we can poll the thread later
                let handle = spawn_blocking(move || {
                    let output = this.remove_blocks_above(new_tip_num);
                    sender.send(output).unwrap();
                });

                this.active_writer_thread = Some(handle);

                Poll::Ready(Some(PersistenceOutput::AddBlocksAbove(receiver)))
            }
            PersistenceAction::SaveFinalizedBlocks(blocks) => {
                if blocks.is_empty() {
                    todo!("return error or something");
                }
                let last_block_hash = blocks.last().unwrap().block().hash();

                // spawn blocking so we can poll the thread later
                let handle = spawn_blocking(move || {
                    this.write(blocks);
                });

                this.active_writer_thread = Some(handle);

                Poll::Ready(Some(PersistenceOutput::RemoveBlocksBefore(last_block_hash)))
            }
        }
    }
}

/// A signal to the persistence task that part of the tree state can be persisted.
pub enum PersistenceAction {
    /// The section of tree state that should be persisted. These blocks are expected in order of
    /// increasing block number.
    SaveFinalizedBlocks(Vec<ExecutedBlock>),

    /// Removes the blocks above the given block number from the database.
    RemoveBlocksAbove(u64),
}

/// An output of the persistence task, that tells the tree that it needs something.
pub enum PersistenceOutput {
    /// Tells the consumer that it can remove the blocks before the given hash, as they have been
    /// persisted.
    RemoveBlocksBefore(B256),

    /// Tells the consumer that the following blocks have been un-persisted, or removed from the
    /// datbase, and they should be re-added to any in memory data structures.
    AddBlocksAbove(oneshot::Receiver<Vec<ExecutedBlock>>),
}
