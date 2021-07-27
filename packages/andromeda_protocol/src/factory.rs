use andromeda_modules::modules::ModuleDefinition;
use cosmwasm_std::HumanAddr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub token_code_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    //Create new token
    Create {
        name: String,
        symbol: String,
        extensions: Vec<ModuleDefinition>,
    },
    //Called by instantiated token contract to store address
    TokenCreationHook {
        symbol: String,
        creator: HumanAddr,
    },
    UpdateAddress {
        symbol: String,
        new_address: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetAddress { symbol: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AddressResponse {
    pub address: HumanAddr,
}
