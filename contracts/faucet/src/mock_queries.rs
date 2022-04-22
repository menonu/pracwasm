use std::collections::HashMap;

use cosmwasm_std::{
    from_binary, from_slice, testing::MockQuerier, to_binary, Addr, Binary, Empty, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use cw20::{BalanceResponse, Cw20QueryMsg};

#[derive(Debug, Clone)]
pub struct Cw20Balance {
    pub contract_address: Addr,
    pub balances: HashMap<Addr, Uint128>,
}

struct WasmQuerier {
    balances: HashMap<Addr, HashMap<Addr, Uint128>>,
}

impl Default for WasmQuerier {
    fn default() -> Self {
        Self::new(None)
    }
}

impl WasmQuerier {
    pub fn new(cw20_balances: Option<&[Cw20Balance]>) -> Self {
        let mut map = HashMap::new();
        if let Some(balances) = cw20_balances {
            for item in balances.iter() {
                map.insert(item.contract_address.clone(), item.balances.clone());
            }
        }

        Self { balances: map }
    }

    fn query(&self, request: &WasmQuery) -> QuerierResult {
        let return_notfound = |addr: &String| {
            SystemResult::Err(SystemError::NoSuchContract {
                addr: addr.to_string(),
            })
        };

        match request {
            WasmQuery::Smart { contract_addr, msg } => self.dispatch(contract_addr, msg),
            WasmQuery::Raw { contract_addr, .. } => return_notfound(contract_addr),
            // WasmQuery::ContractInfo { contract_addr, .. } => return_notfound(contract_addr),
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: "Unknown WasmQuery".to_string(),
            }),
        }
    }

    fn dispatch(&self, contract_address: &str, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            Cw20QueryMsg::Balance { address } => {
                let balances: &HashMap<Addr, Uint128> =
                    match self.balances.get(&Addr::unchecked(contract_address)) {
                        Some(balances) => balances,
                        None => {
                            return SystemResult::Err(SystemError::Unknown {});
                        }
                    };

                let balance = match balances.get(&Addr::unchecked(address)) {
                    Some(v) => v,
                    None => {
                        return SystemResult::Err(SystemError::Unknown {});
                    }
                };

                SystemResult::Ok(to_binary(&BalanceResponse { balance: *balance }).into())
            }
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: msg.to_string(),
            }),
        }
    }
}

pub struct WasmMockQuerier {
    default_querier: MockQuerier,
    wasm_querier: WasmQuerier,
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn new(cw20_balances: Option<&[Cw20Balance]>) -> Self {
        WasmMockQuerier {
            // default_querier: MockQuerier::default(),
            default_querier: MockQuerier::new(&[]),
            wasm_querier: WasmQuerier::new(cw20_balances),
        }
    }

    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(msg) => self.wasm_querier.query(msg),
            _ => self.default_querier.handle_query(request),
        }
    }
}

impl Default for WasmMockQuerier {
    fn default() -> Self {
        Self::new(None)
    }
}
