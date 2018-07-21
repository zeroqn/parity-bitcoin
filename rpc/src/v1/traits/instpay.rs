use jsonrpc_core::Error;
use jsonrpc_macros::Trailing;

use v1::types::H256;
use v1::types::RawTransaction;
use v1::types::TransactionInput;
use v1::types::TransactionOutputs;

build_rpc_trait! {
    pub trait InstPay {
        #[rpc(name = "createpay2phonetransaction")]
        fn create_pay2phone_transaction(&self, Vec<TransactionInput>, TransactionOutputs, Trailing<u32>) -> Result<RawTransaction, Error>;
        #[rpc(name = "getphonepubaddress")]
        fn get_phone_pub_address(&self, String) -> Result<String, Error>;
        #[rpc(name = "pay2phone")]
        fn pay_to_phone(&self, String, f64) -> Result<H256, Error>;
        #[rpc(name = "getbalance")]
        fn get_balance(&self, String) -> Result<String, Error>;
    }
}
