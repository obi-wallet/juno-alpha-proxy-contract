use cosmwasm_std::{Addr, Deps, Event, StdError, StdResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `Signer` type identifies a member of the admin multisig, and its type.
/// The format or encryption of type, `ty`, is up to the client.
/// `address` is verified using the deps API when Signer is created.
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
    /// Constructs a new `Signer`. Intended for use in a `Signers`, which is a wrapped
    /// `Vec<Signer>` with its own methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use obi_proxy_contract::signers::Signer;
    /// use cosmwasm_std::testing::mock_dependencies;
    /// let deps = mock_dependencies();
    ///
    /// let signer = Signer::new(deps.as_ref(), "cosmos1aaa83p76w8x7wdt81w7aaa".to_string(), "nfc".to_string()).unwrap();
    ///
    /// assert_eq!(signer.address(), "cosmos1aaa83p76w8x7wdt81w7aaa".to_string());
    /// assert_eq!(signer.ty(), "nfc".to_string());
    /// ```
    pub fn new(deps: Deps, address: String, ty: String) -> StdResult<Self> {
        Ok(Self {
            address: deps.api.addr_validate(&address)?,
            ty,
        })
    }

    /// Returns this `Signer`'s type String `ty`.
    pub fn ty(&self) -> String {
        self.ty.clone()
    }

    /// Returns this `Signer`'s `address`, converted to String.
    pub fn address(&self) -> String {
        self.address.to_string()
    }
}

impl Signers {
    /// Constructs a new `Signers`, which is a wrapped
    /// `Vec<Signer>` with its own methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use obi_proxy_contract::signers::Signers;
    /// use cosmwasm_std::testing::mock_dependencies;
    /// let deps = mock_dependencies();
    ///
    /// let wrapped_signers = Signers::new(
    ///     deps.as_ref(),
    ///     vec![
    ///         "cosmos1aaa83p76w8x7wdt81w7aaa".to_string(),
    ///         "cosmos1bbbvt76vra972jfbtljbbb".to_string()
    ///     ],
    ///     vec![
    ///         "nfc".to_string(),
    ///         "telegram".to_string(),
    ///     ]
    /// ).unwrap();
    ///
    /// assert_eq!(wrapped_signers.signers()[0].address(), "cosmos1aaa83p76w8x7wdt81w7aaa".to_string());
    /// assert_eq!(wrapped_signers.signers()[1].ty(), "telegram".to_string());
    /// ```
    pub fn new(
        deps: Deps,
        signers: Vec<String>,
        signer_types: Vec<String>,
    ) -> Result<Self, StdError> {
        // Currently this will panic if address validation fails, due to unwrap() in map.
        let signers: Vec<Signer> = signers
            .into_iter()
            .zip(signer_types)
            .map(|signer| Signer::new(deps, signer.0, signer.1))
            .collect::<Result<Vec<Signer>, StdError>>()?;
        Ok(Self { signers })
    }

    /// Returns an `(Event, bool)`, where the `Event` contains all of the signer addresses,
    /// and the `bool` indicates whether a delay should be activated if updating signers.
    /// Intended to make instantiate/confirm_update_admin transactions indexable by signer address.
    ///
    /// # Examples
    ///
    /// ```
    /// use obi_proxy_contract::signers::Signers;
    /// use cosmwasm_std::testing::mock_dependencies;
    /// let deps = mock_dependencies();
    ///
    /// let wrapped_signers = Signers::new(
    ///     deps.as_ref(),
    ///     vec![
    ///         "cosmos1aaa83p76w8x7wdt81w7aaa".to_string(),
    ///         "cosmos1bbbvt76vra972jfbtljbbb".to_string()
    ///     ],
    ///     vec![
    ///         "nfc".to_string(),
    ///         "telegram".to_string(),
    ///     ]
    /// ).unwrap();
    ///
    /// let (event_to_emit, activate_delay) = wrapped_signers.create_event();
    /// assert_eq!(event_to_emit.attributes[0].key, "signer".to_string());
    /// assert_eq!(event_to_emit.attributes[0].value, "cosmos1aaa83p76w8x7wdt81w7aaa".to_string());
    /// assert_eq!(event_to_emit.attributes[1].value, "cosmos1bbbvt76vra972jfbtljbbb".to_string());
    /// assert_eq!(activate_delay, false);
    /// ```
    pub fn create_event(&self) -> (Event, bool) {
        let mut activate_delay = false;
        let mut signers_event = Event::new("obisign");
        for signer in self.signers.clone() {
            // this address temporarily hardcoded
            if signer.address.to_string()
                == *"juno17w77rnps59cnallfskg42s3ntnlhrzu2mjkr3e".to_string()
            {
                activate_delay = true;
            }
            signers_event = signers_event.add_attribute("signer", signer.address);
        }
        (signers_event, activate_delay)
    }

    pub fn signers(&self) -> Vec<Signer> {
        self.signers.clone()
    }
}
