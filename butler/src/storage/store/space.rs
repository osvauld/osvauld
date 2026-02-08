//! Space table operations

use redb::{ReadableTable, ReadableDatabase};
use crate::error::{ButlerError, Result};
use crate::models::SpaceData;
use tracing::instrument;
use super::{RedbStore, SPACES};

impl RedbStore {
    #[instrument(skip_all)]
    pub fn put_space(&self, space: &SpaceData) -> Result<()> {
        let key = &space.meta.id;
        let value = bincode::serialize(space)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut spaces = write_txn.open_table(SPACES)?;
            spaces.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    #[instrument(skip_all)]
    pub fn get_space(&self, space_id: &str) -> Result<Option<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        match table.get(space_id)? {
            Some(guard) => {
                let bytes = guard.value();
                let space: SpaceData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(space))
            }
            None => Ok(None),
        }
    }

    #[instrument(skip_all)]
    pub fn delete_space(&self, space_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut spaces = write_txn.open_table(SPACES)?;
            let result = spaces.remove(space_id)?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    #[instrument(skip_all)]
    pub fn list_spaces(&self) -> Result<Vec<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        let mut spaces = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let space: SpaceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            spaces.push(space);
        }
        Ok(spaces)
    }

    #[instrument(skip_all)]
    pub fn list_child_spaces(&self, parent_id: &str) -> Result<Vec<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        let mut spaces = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let space: SpaceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if space.meta.parent_space_id.as_deref() == Some(parent_id) {
                spaces.push(space);
            }
        }
        Ok(spaces)
    }

    #[instrument(skip_all)]
    pub fn list_root_spaces(&self) -> Result<Vec<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        let mut spaces = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let space: SpaceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if space.meta.parent_space_id.is_none() {
                spaces.push(space);
            }
        }
        Ok(spaces)
    }
}
