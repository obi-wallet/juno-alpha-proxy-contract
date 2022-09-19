use crate::{
    constants::{
        MAINNET_AXLUSDC_IBC, MAINNET_DENOM, MAINNET_DEX_DENOM, TESTNET_DENOM,
        TESTNET_DUMMY_CONTRACT,
    },
    pair_contract::{PairContract, PairMessageType},
};

pub fn get_mainnet_pair_contracts() -> [PairContract; 3] {
    [
        PairContract {
            contract_addr: String::from(
                "juno1utkr0ep06rkxgsesq6uryug93daklyd6wneesmtvxjkz0xjlte9qdj2s8q",
            ),
            denom1: String::from(MAINNET_AXLUSDC_IBC),
            denom2: String::from(MAINNET_DEX_DENOM),
            query_format: PairMessageType::LoopType,
        },
        PairContract {
            contract_addr: String::from(
                "juno1qc8mrs3hmxm0genzrd92akja5r0v7mfm6uuwhktvzphhz9ygkp8ssl4q07",
            ),
            denom1: String::from(MAINNET_DENOM),
            denom2: String::from(MAINNET_DEX_DENOM),
            query_format: PairMessageType::LoopType,
        },
        PairContract {
            contract_addr: String::from(
                "juno1ctsmp54v79x7ea970zejlyws50cj9pkrmw49x46085fn80znjmpqz2n642",
            ),
            denom1: String::from(MAINNET_DENOM),
            denom2: String::from(MAINNET_AXLUSDC_IBC),
            query_format: PairMessageType::JunoType,
        },
    ]
}

pub fn get_testnet_pair_contracts() -> [PairContract; 3] {
    [
        PairContract {
            contract_addr: String::from(TESTNET_DUMMY_CONTRACT),
            denom1: String::from(MAINNET_AXLUSDC_IBC),
            denom2: String::from(MAINNET_DEX_DENOM),
            query_format: PairMessageType::LoopType,
        },
        PairContract {
            contract_addr: String::from(TESTNET_DUMMY_CONTRACT),
            denom1: String::from(TESTNET_DENOM),
            denom2: String::from(MAINNET_DEX_DENOM),
            query_format: PairMessageType::LoopType,
        },
        PairContract {
            contract_addr: String::from(TESTNET_DUMMY_CONTRACT),
            denom1: String::from(TESTNET_DENOM),
            denom2: String::from(MAINNET_AXLUSDC_IBC),
            query_format: PairMessageType::JunoType,
        },
    ]
}

pub fn get_local_pair_contracts() -> [PairContract; 3] {
    [
        PairContract {
            contract_addr: String::from("local_usdc_to_uloop_fake"),
            denom1: String::from(MAINNET_AXLUSDC_IBC),
            denom2: String::from(MAINNET_DEX_DENOM),
            query_format: PairMessageType::LoopType,
        },
        PairContract {
            contract_addr: String::from("local_ujuno_to_uloop_fake"),
            denom1: String::from("testtokens"),
            denom2: String::from(MAINNET_DEX_DENOM),
            query_format: PairMessageType::LoopType,
        },
        PairContract {
            contract_addr: String::from("local_ujuno_to_usdc_fake"),
            denom1: String::from("testtokens"),
            denom2: String::from(MAINNET_AXLUSDC_IBC),
            query_format: PairMessageType::JunoType,
        },
    ]
}
