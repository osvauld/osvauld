use super::capability::{ResourceTokenType, Role};
use super::ucan_domain::{DelegationTemplate, ResourceUcan, ResourceUcanError, ResourceUcanResult};

/// Resource owner token (full control)
#[derive(Debug, Clone)]
pub struct ResourceOwnerToken {
    ucan: ResourceUcan,
}

impl ResourceOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::ResourceOwner {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected ResourceOwner, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Owner {
            return Err(ResourceUcanError::ValidationFailed(
                format!("ResourceOwner must have Owner role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn resource_id(&self) -> String {
        self.ucan.resource_id().expect("ResourceOwner must have resource_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }
}

/// Resource share token (for Node or User role)
#[derive(Debug, Clone)]
pub struct ResourceShareToken {
    ucan: ResourceUcan,
}

impl ResourceShareToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::ResourceShare {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected ResourceShare, got {:?}", ucan.token_type())
            ));
        }

        if !matches!(ucan.role(), Role::Node | Role::User) {
            return Err(ResourceUcanError::ValidationFailed(
                format!("ResourceShare must have Node or User role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn role(&self) -> Role {
        self.ucan.role()
    }

    pub fn resource_id(&self) -> String {
        self.ucan.resource_id().expect("ResourceShare must have resource_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }
}

/// Resource viewer token (for Viewer role)
#[derive(Debug, Clone)]
pub struct ResourceViewerToken {
    ucan: ResourceUcan,
}

impl ResourceViewerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::ResourceViewer {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected ResourceViewer, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Viewer {
            return Err(ResourceUcanError::ValidationFailed(
                format!("ResourceViewer must have Viewer role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn resource_id(&self) -> String {
        self.ucan.resource_id().expect("ResourceViewer must have resource_id")
    }
}

/// Folder owner token (full control)
#[derive(Debug, Clone)]
pub struct FolderOwnerToken {
    ucan: ResourceUcan,
}

impl FolderOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::FolderOwner {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected FolderOwner, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Owner {
            return Err(ResourceUcanError::ValidationFailed(
                format!("FolderOwner must have Owner role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn folder_id(&self) -> String {
        self.ucan.folder_id().expect("FolderOwner must have folder_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }
}

/// Folder share token (for Node or User role)
#[derive(Debug, Clone)]
pub struct FolderShareToken {
    ucan: ResourceUcan,
}

impl FolderShareToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::FolderShare {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected FolderShare, got {:?}", ucan.token_type())
            ));
        }

        if !matches!(ucan.role(), Role::Node | Role::User) {
            return Err(ResourceUcanError::ValidationFailed(
                format!("FolderShare must have Node or User role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn role(&self) -> Role {
        self.ucan.role()
    }

    pub fn folder_id(&self) -> String {
        self.ucan.folder_id().expect("FolderShare must have folder_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }
}

/// Folder viewer token (for Viewer role)
#[derive(Debug, Clone)]
pub struct FolderViewerToken {
    ucan: ResourceUcan,
}

impl FolderViewerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::FolderViewer {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected FolderViewer, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Viewer {
            return Err(ResourceUcanError::ValidationFailed(
                format!("FolderViewer must have Viewer role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn folder_id(&self) -> String {
        self.ucan.folder_id().expect("FolderViewer must have folder_id")
    }
}
