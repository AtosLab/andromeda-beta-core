use cosmwasm_std::{Api, Env, Extern, HumanAddr, Querier, StdError, StdResult, Storage};
use cosmwasm_storage::{bucket, bucket_read};

use crate::{
    common::{is_unique, require},
    modules::{HookResponse, Module, ModuleDefinition},
};

const WHITELIST_NS: &[u8] = b"whitelist";

pub struct Whitelist {
    pub moderators: Vec<HumanAddr>,
}

impl Whitelist {
    pub fn is_moderator(&self, addr: &HumanAddr) -> bool {
        self.moderators.contains(addr)
    }
    pub fn whitelist_addr<S: Storage>(&self, storage: &mut S, addr: &HumanAddr) -> StdResult<()> {
        bucket(WHITELIST_NS, storage).save(addr.to_string().as_bytes(), &true)
    }
    pub fn remove_whitelist<S: Storage>(&self, storage: &mut S, addr: &HumanAddr) -> StdResult<()> {
        bucket(WHITELIST_NS, storage).save(addr.to_string().as_bytes(), &false)
    }
    pub fn is_whitelisted<S: Storage>(&self, storage: &S, addr: &HumanAddr) -> StdResult<bool> {
        match bucket_read(WHITELIST_NS, storage).load(addr.to_string().as_bytes()) {
            Ok(whitelisted) => Ok(whitelisted),
            Err(e) => match e {
                cosmwasm_std::StdError::NotFound { .. } => Ok(false),
                _ => Err(e),
            },
        }
    }
}

impl Module for Whitelist {
    fn validate(&self, all_modules: Vec<ModuleDefinition>) -> StdResult<bool> {
        require(
            is_unique(self, &all_modules),
            StdError::generic_err("Whitelist module must be unique"),
        )?;

        Ok(true)
    }
    fn as_definition(&self) -> ModuleDefinition {
        ModuleDefinition::WhiteList {
            moderators: self.moderators.to_vec(),
        }
    }
    fn pre_publish<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &mut Extern<S, A, Q>,
        env: Env,
        _token_id: i64,
    ) -> StdResult<HookResponse> {
        require(
            self.is_whitelisted(&deps.storage, &env.message.sender.clone())?,
            StdError::unauthorized(),
        )?;

        Ok(HookResponse::default())
    }
}

mod test {
    use super::*;

    #[test]
    fn test_validate() {
        let wl = Whitelist { moderators: vec![] };
        let mut modules = vec![
            wl.as_definition().clone(),
            ModuleDefinition::Taxable {
                tax: 2,
                receivers: vec![],
            },
        ];

        assert_eq!(wl.validate(modules.to_vec()), Ok(true));

        modules.push(ModuleDefinition::WhiteList { moderators: vec![] });

        assert_eq!(
            wl.validate(modules.to_vec()),
            Err(StdError::generic_err("Whitelist module must be unique"))
        );
    }
}