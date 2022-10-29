#[cfg(test)]
mod tests {
    use crate::signers::Signers;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{Attribute, Event};

    #[test]
    fn store_signers() {
        let deps = mock_dependencies();
        let these_signers = vec!["signer1".to_string(), "signer2".to_string()];
        let these_types = vec!["nfc".to_string(), "telegram".to_string()];
        let signers =
            Signers::new(deps.as_ref(), these_signers.clone(), these_types.clone()).unwrap();
        assert_eq!(signers.signers()[0].address(), these_signers[0].clone());
        assert_eq!(signers.signers()[1].address(), these_signers[1].clone());
        assert_eq!(signers.signers()[0].ty(), these_types[0]);
        assert_eq!(signers.signers()[1].ty(), these_types[1]);
    }

    #[test]
    fn create_signers_event() {
        let deps = mock_dependencies();
        let these_signers = vec!["signer1".to_string(), "signer2".to_string()];
        let these_types = vec!["nfc".to_string(), "telegram".to_string()];
        let signers =
            Signers::new(deps.as_ref(), these_signers.clone(), these_types.clone()).unwrap();

        let (test_event, should_delay) = signers.create_event();
        assert_eq!(should_delay, false);
        assert_eq!(
            test_event,
            Event::new("obisign").add_attributes(vec![
                Attribute::new("signer", these_signers[0].clone()),
                Attribute::new("signer", these_signers[1].clone())
            ])
        );
    }

    #[test]
    fn create_signers_event_with_delay() {
        let deps = mock_dependencies();
        let these_signers = vec![
            "signer1".to_string(),
            "juno17w77rnps59cnallfskg42s3ntnlhrzu2mjkr3e".to_string(),
        ];
        let these_types = vec!["nfc".to_string(), "obi".to_string()];
        let signers =
            Signers::new(deps.as_ref(), these_signers.clone(), these_types.clone()).unwrap();

        let (test_event, should_delay) = signers.create_event();
        assert_eq!(should_delay, true);
        assert_eq!(
            test_event,
            Event::new("obisign").add_attributes(vec![
                Attribute::new("signer", these_signers[0].clone()),
                Attribute::new("signer", these_signers[1].clone())
            ])
        );
    }
}
