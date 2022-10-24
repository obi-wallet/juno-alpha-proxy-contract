use cosmwasm_std::{Deps, Addr, StdResult, StdError, Event};
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct Signer {
    address: Addr,
    ty: String, // arbitrary how this is set up by client
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct Signers {
    signers: Vec<Signer>,
}

impl Signer {
    pub fn new(deps: Deps, address: String, ty: String) -> StdResult<Self> {
        Ok(
            Self {
                address: deps.api.addr_validate(&address)?,
                ty,
            }
        )
    }

    pub fn ty(&self) -> String {
        self.ty.clone()
    }
}

impl Signers {
    pub fn new(deps: Deps, signers: Vec<String>, signer_types: Vec<String>) -> Result<Self, StdError> {
        // Currently this will panic if address validation fails, due to unwrap() in map.
        let signers: Vec<Signer> = signers.into_iter().zip(signer_types).map(|signer| Signer::new(
            deps,
            signer.0,
            signer.1,
        )).collect::<Result<Vec<Signer>, StdError>>()?;
        Ok( Self { signers })
    }

    pub fn create_event(&self) -> (Event, bool) {
        let mut activate_delay = false;
        let mut signers_event = Event::new("obisign");
        for signer in self.signers.clone() {
            // this address temporarily hardcoded
            if signer.address.to_string() == *"juno17w77rnps59cnallfskg42s3ntnlhrzu2mjkr3e".to_string() {
                activate_delay = true;
            }
            signers_event =
                signers_event.add_attribute("signer", signer.address);
        }
        (signers_event, activate_delay)
    }
}