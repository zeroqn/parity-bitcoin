use chain::Transaction as GlobalTransaction;

use jsonrpc_core::Error;
use jsonrpc_macros::Trailing;

use primitives::bytes::Bytes as GlobalBytes;
use primitives::hash::H256 as GlobalH256;

use ser::serialize;
use std::process::Command;
use std::str::FromStr;
use sync;

use keys::{KeyPair, Network, Private, Secret};
use v1::helpers::errors::execution;
use v1::traits::InstPay;
use v1::types::H256;
use v1::types::{RawTransaction, TransactionInput, TransactionOutput, TransactionOutputs};

pub struct InstPayClient<T: InstPayClientCoreApi> {
    core: T,
}

pub trait InstPayClientCoreApi: Send + Sync + 'static {
    fn create_pay2phone_transaction(
        &self,
        inputs: Vec<TransactionInput>,
        outputs: TransactionOutputs,
        lock_time: Trailing<u32>,
    ) -> Result<GlobalTransaction, String>;
    fn get_phone_pub_address(&self, phone: String) -> Result<String, Error>;
    fn pay_to_phone(&self, phone: String, amount: f64) -> Result<H256, Error>;
    fn get_balance(&self, account: String) -> Result<String, Error>;
}

pub struct InstPayClientCore {
    local_sync_node: sync::LocalNodeRef,
}

impl InstPayClientCore {
    pub fn new(local_sync_node: sync::LocalNodeRef) -> Self {
        InstPayClientCore {
            local_sync_node: local_sync_node,
        }
    }

    pub fn gen_keypair_from_phone(phone: &str) -> KeyPair {
        use ring::digest;
        let digested = digest::digest(&digest::SHA512, &phone.as_bytes());
        let mut secret = Secret::default();
        secret.copy_from_slice(&(digested.as_ref()[0..32]));
        let private = Private {
            network: Network::Testnet,
            secret: secret,
            compressed: false,
        };
        KeyPair::from_private(private).unwrap()
    }

    pub fn do_pay_to_phone(phone: &str, amount: f64) -> Result<H256, Error> {
        use chain;
        use std::process;

        let kp = Self::gen_keypair_from_phone(phone);
        println!("{:?}", kp);
        let address = kp.address().to_string();
        let _amount_in_satoshis = (amount * (chain::constants::SATOSHIS_IN_COIN as f64)) as u64;

        let output = Command::new("bitcoin-cli")
            .args(&["-regtest", "sendtoaddress", &address, &amount.to_string()])
            .stdout(process::Stdio::piped())
            .output()
            .expect("fail to send payment");

        let h256_str = String::from_utf8_lossy(&output.stdout);
        let h256_str = h256_str.trim_right_matches('\n');
        Ok(H256::from_str(h256_str).unwrap())
    }

    pub fn do_create_pay2phone_transaction(
        inputs: Vec<TransactionInput>,
        outputs: TransactionOutputs,
        lock_time: Trailing<u32>,
    ) -> Result<GlobalTransaction, String> {
        use chain;
        use global_script::Builder as ScriptBuilder;

        // to make lock_time work at least one input must have sequnce < SEQUENCE_FINAL
        let lock_time = lock_time.unwrap_or_default();
        let default_sequence = if lock_time != 0 {
            chain::constants::SEQUENCE_FINAL - 1
        } else {
            chain::constants::SEQUENCE_FINAL
        };

        // prepare inputs
        let inputs: Vec<_> = inputs
            .into_iter()
            .map(|input| chain::TransactionInput {
                previous_output: chain::OutPoint {
                    hash: Into::<GlobalH256>::into(input.txid).reversed(),
                    index: input.vout,
                },
                script_sig: GlobalBytes::new(), // default script
                sequence: input.sequence.unwrap_or(default_sequence),
                script_witness: vec![],
            })
            .collect();

        // prepare outputs
        let outputs: Vec<_> = outputs
            .outputs
            .into_iter()
            .map(|output| match output {
                TransactionOutput::Address(_with_address) => unimplemented!(),
                TransactionOutput::ScriptData(_with_script_data) => unimplemented!(),
                TransactionOutput::Phone(with_phone) => {
                    let amount_in_satoshis =
                        (with_phone.amount * (chain::constants::SATOSHIS_IN_COIN as f64)) as u64;
                    let kp = InstPayClientCore::gen_keypair_from_phone(&with_phone.phone);
                    let script = ScriptBuilder::build_p2pkh(&kp.address().hash);

                    chain::TransactionOutput {
                        value: amount_in_satoshis,
                        script_pubkey: script.to_bytes(),
                    }
                }
            })
            .collect();

        // now construct && serialize transaction
        let transaction = GlobalTransaction {
            version: 1,
            inputs: inputs,
            outputs: outputs,
            lock_time: lock_time,
        };

        Ok(transaction)
    }
}

impl InstPayClientCoreApi for InstPayClientCore {
    fn create_pay2phone_transaction(
        &self,
        inputs: Vec<TransactionInput>,
        outputs: TransactionOutputs,
        lock_time: Trailing<u32>,
    ) -> Result<GlobalTransaction, String> {
        InstPayClientCore::do_create_pay2phone_transaction(inputs, outputs, lock_time)
    }

    fn get_phone_pub_address(&self, phone: String) -> Result<String, Error> {
        let kp = InstPayClientCore::gen_keypair_from_phone(&phone);
        Ok(kp.address().to_string())
    }

    fn pay_to_phone(&self, phone: String, amount: f64) -> Result<H256, Error> {
        InstPayClientCore::do_pay_to_phone(&phone, amount)
    }

    fn get_balance(&self, account: String) -> Result<String, Error> {
        use std::process;

        let output = Command::new("bitcoin-cli")
            .args(&["-regtest", "getbalance", &account])
            .stdout(process::Stdio::piped())
            .output()
            .expect("fail to send payment");

        let output = String::from_utf8_lossy(&output.stdout);
        let output = output.trim_right_matches('\n');
        Ok(output.to_string())
    }
}

impl<T> InstPayClient<T>
where
    T: InstPayClientCoreApi,
{
    pub fn new(core: T) -> Self {
        InstPayClient { core: core }
    }
}

impl<T> InstPay for InstPayClient<T>
where
    T: InstPayClientCoreApi,
{
    fn create_pay2phone_transaction(
        &self,
        inputs: Vec<TransactionInput>,
        outputs: TransactionOutputs,
        lock_time: Trailing<u32>,
    ) -> Result<RawTransaction, Error> {
        // reverse hashes of inputs
        let inputs: Vec<_> = inputs
            .into_iter()
            .map(|mut input| {
                input.txid = input.txid.reversed();
                input
            })
            .collect();
        let transaction = self.core
            .create_pay2phone_transaction(inputs, outputs, lock_time)
            .map_err(|e| execution(e))?;
        let transaction = serialize(&transaction);
        Ok(transaction.into())
    }

    fn get_phone_pub_address(&self, phone: String) -> Result<String, Error> {
        self.core.get_phone_pub_address(phone)
    }

    fn pay_to_phone(&self, phone: String, amount: f64) -> Result<H256, Error> {
        self.core.pay_to_phone(phone, amount)
    }

    fn get_balance(&self, account: String) -> Result<String, Error> {
        self.core.get_balance(account)
    }
}
