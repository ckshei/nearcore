//! Settings of the parameters of the economics.
use near_primitives::transaction::TransactionBody;
use near_primitives::types::Balance;
use wasm::types::Config;

/// The structure that holds the parameters of the economics.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct EconomicsConfig {
    /// The cost to store one byte of storage per block.
    pub storage_cost_byte_per_block: Balance,
    pub transactions_costs: TransactionsCosts,
    /// Config of wasm operations.
    pub wasm_config: Config,
}

/// The costs of the transactions.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct TransactionsCosts {
    pub create_account: Balance,
    pub deploy_contract: Balance,
    pub function_call: Balance,
    pub self_function_call: Balance,
    pub send_money: Balance,
    pub stake: Balance,
    pub swap_key: Balance,
    pub add_key: Balance,
    pub delete_key: Balance,
}

impl TransactionsCosts {
    /// Get the cost of the given transaction.
    pub fn cost(&self, transaction_body: &TransactionBody) -> Balance {
        use TransactionBody::*;
        match transaction_body {
            CreateAccount(_) => self.create_account.clone(),
            DeployContract(_) => self.deploy_contract.clone(),
            FunctionCall(_)
                if Some(transaction_body.get_originator())
                    == transaction_body.get_contract_id() =>
            {
                self.self_function_call.clone()
            }
            FunctionCall(_) => self.function_call.clone(),
            SendMoney(_) => self.send_money.clone(),
            Stake(_) => self.stake.clone(),
            SwapKey(_) => self.swap_key.clone(),
            AddKey(_) => self.add_key.clone(),
            DeleteKey(_) => self.delete_key.clone(),
        }
    }
}
