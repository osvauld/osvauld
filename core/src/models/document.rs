use std::future::Future;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{AsyncTransact, Doc, ReadTxn, StateVector, Update};
/// Create a new YJS document
///
/// # Returns
/// * `Doc` - A new YJS document ready for use
pub fn create_doc() -> Doc {
    Doc::new()
}

/// YJS Document implementation with v2 encoding methods
impl YjsDocExt for Doc {
    async fn apply_update_v2(&mut self, update: &[u8]) -> Result<(), String> {
        if update.is_empty() {
            log::debug!("Skipping empty update");
            return Ok(());
        }

        log::debug!("Applying v2 update, size: {} bytes, first bytes: {:?}", 
            update.len(), 
            &update[..update.len().min(10)]
        );

        // Decode v2 update
        let update_obj = Update::decode_v2(update).map_err(|e| {
            log::error!(
                "Failed to decode v2 update: {:?}, update size: {}, update bytes: {:?}",
                e,
                update.len(),
                update
            );
            format!("Failed to decode v2 update: {:?}", e)
        })?;

        // Apply the update to the document
        let mut txn = self.transact_mut().await;
        txn.apply_update(update_obj)
            .map_err(|e| format!("Failed to apply update to document: {:?}", e))?;

        Ok(())
    }

    async fn get_state_vector_v2(&self) -> Vec<u8> {
        let txn = self.transact().await;
        txn.state_vector().encode_v2()
    }

    async fn get_state_as_update_v2(&self) -> Vec<u8> {
        let txn = self.transact().await;
        txn.encode_state_as_update_v2(&StateVector::default())
    }

    async fn get_diff_update_v2(&self, state_vector: &[u8]) -> Result<Vec<u8>, String> {
        log::debug!("Getting diff update v2, state vector size: {} bytes", state_vector.len());
        
        if state_vector.is_empty() {
            log::warn!("Empty state vector provided, returning full state");
            return Ok(self.get_state_as_update_v2().await);
        }

        // Decode the state vector
        let sv = StateVector::decode_v2(state_vector).map_err(|e| {
            log::error!(
                "Failed to decode v2 state vector: {:?}, size: {}, bytes: {:?}",
                e,
                state_vector.len(),
                state_vector
            );
            format!("Failed to decode v2 state vector: {:?}", e)
        })?;

        // Generate diff update
        let txn = self.transact().await;
        let diff = txn.encode_diff_v2(&sv);
        log::debug!("Generated diff update, size: {} bytes", diff.len());
        Ok(diff)
    }
}
pub trait YjsDocExt {
    /// Apply a v2 encoded update to the document
    ///
    /// # Arguments
    /// * `update` - The v2 encoded update as bytes
    ///
    /// # Returns
    /// * `Result<(), String>` - Success or error message
    fn apply_update_v2(&mut self, update: &[u8])
    -> impl Future<Output = Result<(), String>> + Send;

    /// Get the state vector of the document encoded in v2 format
    ///
    /// # Returns
    /// * `Vec<u8>` - The v2 encoded state vector
    fn get_state_vector_v2(&self) -> impl Future<Output = Vec<u8>> + Send;

    /// Get the full document state as a v2 encoded update
    ///
    /// # Returns
    /// * `Vec<u8>` - The v2 encoded document state
    fn get_state_as_update_v2(&self) -> impl Future<Output = Vec<u8>> + Send;

    /// Get a diff update between the current state and the provided state vector
    ///
    /// # Arguments
    /// * `state_vector` - The v2 encoded state vector to diff against
    ///
    /// # Returns
    /// * `Result<Vec<u8>, String>` - The v2 encoded diff update or error
    fn get_diff_update_v2(
        &self,
        state_vector: &[u8],
    ) -> impl Future<Output = Result<Vec<u8>, String>> + Send;
}
