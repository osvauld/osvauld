//! Contact and Device table operations

use redb::{ReadableTable, ReadableDatabase};
use crate::error::{ButlerError, Result};
use crate::models::{ContactData, DeviceData};
use super::{RedbStore, CONTACTS, DEVICES};

impl RedbStore {
    // =========================================================================
    // Device Operations
    // Key: {device_id}
    // =========================================================================

    pub fn put_device(&self, device: &DeviceData) -> Result<()> {
        let value = bincode::serialize(device)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(DEVICES)?;
            table.insert(device.id.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_device(&self, device_id: &str) -> Result<Option<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        match table.get(device_id)? {
            Some(guard) => {
                let bytes = guard.value();
                let device: DeviceData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(device))
            }
            None => Ok(None),
        }
    }

    pub fn delete_device(&self, device_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(DEVICES)?;
            let result = table.remove(device_id)?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        let mut devices = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let device: DeviceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            devices.push(device);
        }
        Ok(devices)
    }

    pub fn get_current_device(&self) -> Result<Option<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        for result in table.iter()? {
            let (_, value) = result?;
            let device: DeviceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if device.is_current {
                return Ok(Some(device));
            }
        }
        Ok(None)
    }

    pub fn get_node_device(&self) -> Result<Option<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        for result in table.iter()? {
            let (_, value) = result?;
            let device: DeviceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if device.is_node {
                return Ok(Some(device));
            }
        }
        Ok(None)
    }

    // =========================================================================
    // Contact Operations
    // Key: {user_did}
    // =========================================================================

    pub fn put_contact(&self, contact: &ContactData) -> Result<()> {
        let value = bincode::serialize(contact)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CONTACTS)?;
            table.insert(contact.did.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_contact(&self, user_did: &str) -> Result<Option<ContactData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONTACTS)?;

        match table.get(user_did)? {
            Some(guard) => {
                let bytes = guard.value();
                let contact: ContactData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(contact))
            }
            None => Ok(None),
        }
    }

    pub fn delete_contact(&self, user_did: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(CONTACTS)?;
            let result = table.remove(user_did)?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_contacts(&self) -> Result<Vec<ContactData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONTACTS)?;

        let mut contacts = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let contact: ContactData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            contacts.push(contact);
        }
        Ok(contacts)
    }
}
