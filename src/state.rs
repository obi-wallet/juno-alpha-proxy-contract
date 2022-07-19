use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Admin {
    pub admin: String,
}

impl Admin {
    /// returns true if the address is a registered admin
    pub fn is_admin(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.admin == addr
    }
}

pub const ADMIN: Item<Admin> = Item::new("admin");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_admin() {
        let admin: &str = "bob";
        let config = Admin {
            admin: admin.to_string(),
        };

        assert!(config.is_admin(admin.to_string()));
        assert!(!config.is_admin("other".to_string()));
    }
}
