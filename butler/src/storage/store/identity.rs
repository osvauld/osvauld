//! Identity table operations

use redb::{ReadableTable, ReadableDatabase};
use crate::error::{ButlerError, Result};
use crate::models::{EncryptedKeyStore, IdentityData};
use super::{RedbStore, IDENTITY};

impl RedbStore {
    pub fn get_identity(&self) -> Result<Option<IdentityData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(IDENTITY)?;

        match table.get("self")? {
            Some(guard) => {
                let bytes = guard.value();
                let identity: IdentityData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(identity))
            }
            None => Ok(None),
        }
    }

    pub fn set_identity(&self, identity: &IdentityData) -> Result<()> {
        let value = bincode::serialize(identity)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(IDENTITY)?;
            table.insert("self", value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_keystore(&self) -> Result<Option<EncryptedKeyStore>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(IDENTITY)?;

        match table.get("keystore")? {
            Some(guard) => {
                let bytes = guard.value();
                let keystore: EncryptedKeyStore = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(keystore))
            }
            None => Ok(None),
        }
    }

    pub fn set_keystore(&self, keystore: &EncryptedKeyStore) -> Result<()> {
        let value = bincode::serialize(keystore)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(IDENTITY)?;
            table.insert("keystore", value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn is_signed_up(&self) -> Result<bool> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(IDENTITY)?;

        let has_self = table.get("self")?.is_some();
        let has_keystore = table.get("keystore")?.is_some();
        Ok(has_self && has_keystore)
    }
}
